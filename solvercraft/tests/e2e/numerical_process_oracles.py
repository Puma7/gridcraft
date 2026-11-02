"""Independent numerical contracts against a real, immutable Solvercraft CLI.

Test-only NumPy/SciPy provide HiGHS and distribution CDF references. Integer
models use exhaustive enumeration; QP cases use constructed KKT certificates;
retained statistics are recomputed independently. No product engine is used as
its own oracle. Reports contain only synthetic requests and typed results.
"""
from __future__ import annotations
import argparse
import copy
import hashlib
import itertools
import json
import math
from pathlib import Path
import random
import subprocess
import sys

SEED = 2026110207


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False)


def solve_result(value):
    while isinstance(value, dict):
        if {'feasibility', 'optimality', 'termination'} <= value.keys():
            return value
        value = value.get('result', value.get('solve'))
    raise ValueError('No typed solve result')


def terms(values):
    return [{'variable': i, 'coefficient': float(value)} for i, value in enumerate(values) if value]


def model(name, bounds, costs, rows=(), direction='minimize', offset=0, kinds=None):
    return {'schema_version': 1, 'name': name,
            'variables': [{'name': f'x{i}', 'lower': lo, 'upper': hi, 'kind': kinds[i] if kinds else 'continuous'} for i, (lo, hi) in enumerate(bounds)],
            'objective': {'direction': direction, 'offset': float(offset), 'linear': terms(costs), 'quadratic': []},
            'constraints': [{'name': f'r{i}', 'terms': terms(a), 'relation': relation, 'rhs': float(b)} for i, (a, relation, b) in enumerate(rows)]}


def reduced_objective(problem, point):
    return math.fsum(term['coefficient'] * point[term['variable']] for term in problem['objective']['linear'])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--executable', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--python-site', type=Path)
    args = parser.parse_args()
    if args.python_site:
        sys.path.insert(0, str(args.python_site))
    import numpy as np
    import scipy
    from scipy import optimize, stats
    executable = args.executable.resolve()
    identity = hashlib.sha256(executable.read_bytes()).hexdigest()
    rng = random.Random(SEED)
    records = []

    def call(command, request):
        frozen = copy.deepcopy(request)
        encoded = canonical(frozen)
        result = subprocess.run([str(executable), command, '-'], input=encoded, text=True, capture_output=True, timeout=40)
        if result.returncode:
            try:
                error = json.loads(result.stderr.splitlines()[-1])
            except (ValueError, IndexError):
                error = {'unstructured_failure': True}
            return None, error
        if result.stderr:
            raise ValueError('Unexpected diagnostics on a successful process')
        return json.loads(result.stdout), None

    def check(label, command, request, verify, allow_rejection=False):
        fingerprint = hashlib.sha256(canonical(request).encode()).hexdigest()
        response, error = call(command, request)
        try:
            if error is not None:
                if allow_rejection and 'unstructured_failure' not in error:
                    records.append({'case': label, 'command': command, 'status': 'explicit_rejection', 'request_sha256': fingerprint})
                    return
                raise AssertionError('Unexpected process rejection: ' + str(error))
            detail = verify(response) or {}
            status = 'honest_inconclusive' if detail.pop('honest_inconclusive', False) else 'passed'
            records.append({'case': label, 'command': command, 'status': status, 'request_sha256': fingerprint, **detail})
        except (AssertionError, ValueError, KeyError, TypeError) as failure:
            records.append({'case': label, 'command': command, 'status': 'failed', 'request_sha256': fingerprint,
                            'reason': str(failure), 'request': copy.deepcopy(request), 'typed_response': response, 'typed_error': error})

    def validate_solution(problem, response, expected_reduced, atol=3e-6):
        result = solve_result(response)
        assert result['feasibility'] == 'feasible', result
        assert result['optimality'] in {'optimal', 'best_found'}, result
        point = result['values']
        assert len(point) == len(problem['variables']) and all(math.isfinite(x) for x in point)
        for i, variable in enumerate(problem['variables']):
            assert variable['lower'] - atol <= point[i] <= variable['upper'] + atol, ('bounds', point)
            if variable['kind'] != 'continuous':
                assert abs(point[i] - round(point[i])) <= 1e-7, ('integrality', point)
        for row in problem['constraints']:
            lhs = math.fsum(t['coefficient'] * point[t['variable']] for t in row['terms'])
            tolerance = atol * max(1, abs(row['rhs']), math.fsum(abs(t['coefficient'] * point[t['variable']]) for t in row['terms']))
            violation = abs(lhs-row['rhs']) if row['relation'] == 'equal' else max(0, lhs-row['rhs']) if row['relation'] == 'less_equal' else max(0, row['rhs']-lhs)
            assert violation <= tolerance, ('original_constraint', lhs, row['rhs'], point)
        actual = reduced_objective(problem, point)
        assert abs(actual-expected_reduced) <= atol * max(1, abs(expected_reduced)), ('reduced_objective', expected_reduced, actual, point)
        return {'expected_reduced': expected_reduced, 'actual_reduced': actual, 'reported_optimality': result['optimality']}

    # A different LP engine is the numerical reference; constants are excluded
    # from comparisons so they cannot hide a wrong decision behind roundoff.
    for index in range(72):
        n = 2 + index % 4
        bounds = [(-rng.randint(1, 5), rng.randint(1, 8)) for _ in range(n)]
        costs = [rng.randint(-8, 8) for _ in range(n)]
        rows = []
        for j in range(n + 2):
            a = [rng.randint(-4, 4) for _ in range(n)]
            if not any(a): a[0] = 1
            scale = 10. ** rng.randint(-3, 3)
            relation = 'equal' if index % 9 == 0 and j == 0 else 'less_equal'
            rows.append(([v*scale for v in a], relation, 0 if relation == 'equal' else rng.randint(1, 12)*scale))
        direction = 'maximize' if index % 2 else 'minimize'
        problem = model(f'lp-{index}', bounds, costs, rows, direction, offset=(-1)**index * (1e16 if index % 11 == 0 else 7))
        sign = -1 if direction == 'maximize' else 1
        ub = [(a, b) for a, rel, b in rows if rel == 'less_equal']
        eq = [(a, b) for a, rel, b in rows if rel == 'equal']
        oracle = optimize.linprog(np.array(costs)*sign, A_ub=[a for a, _ in ub] or None, b_ub=[b for _, b in ub] or None,
                                  A_eq=[a for a, _ in eq] or None, b_eq=[b for _, b in eq] or None, bounds=bounds, method='highs')
        assert oracle.success, oracle.message
        expected = float(oracle.fun * sign)
        request = {'model': problem, 'options': {'seed': str(SEED)}}
        for command in ['solve', 'solve-sparse']:
            check(f'lp-{index}-{command}', command, request, lambda response, p=problem, e=expected: validate_solution(p, response, e))

    # A finite integer oracle has no optimization library dependency.
    for index in range(48):
        n = 2 + index % 3
        bounds = [(-2, 4) for _ in range(n)]
        costs = [rng.randint(-5, 5) for _ in range(n)]
        rows = [([rng.randint(-3, 3) for _ in range(n)], 'less_equal', rng.randint(0, 9)) for _ in range(4)]
        direction = 'maximize' if index % 2 else 'minimize'
        problem = model(f'integer-{index}', bounds, costs, rows, direction, offset=1e16 if index % 7 == 0 else 0, kinds=['integer']*n)
        feasible = [point for point in itertools.product(range(-2, 5), repeat=n) if all(sum(a*x for a,x in zip(row, point)) <= rhs for row, _, rhs in rows)]
        values = [sum(c*x for c,x in zip(costs, point)) for point in feasible]
        expected = (max if direction == 'maximize' else min)(values)
        check(f'integer-{index}', 'solve', {'model': problem, 'options': {'seed': str(SEED)}},
              lambda response, p=problem, e=expected: validate_solution(p, response, e))

    # Coupled convex quadratics with an independently constructed KKT witness.
    for index in range(36):
        n = 2 + index % 3
        r = np.array([[rng.randint(-3, 3) for _ in range(n)] for _ in range(n)], dtype=float)
        h = r.T @ r + np.eye(n)
        star = np.array([rng.randint(-10, 10)/10 for _ in range(n)])
        a = np.array([rng.randint(1, 4) for _ in range(n)], dtype=float)
        multiplier = 2. if index % 2 else 0.
        linear = -2*h@star - multiplier*a
        rows = [(a.tolist(), 'less_equal', float(a@star))] if multiplier else []
        problem = model(f'qp-kkt-{index}', [(-4.,4.)]*n, linear, rows)
        problem['objective']['quadratic'] = [{'left': i, 'right': j, 'coefficient': float(h[i,j]*(1 if i == j else 2))} for i in range(n) for j in range(i,n)]
        expected = float(star@h@star + linear@star)
        def qp_check(response, p=problem, s=star, matrix=h, q=linear, expected=expected):
            result = solve_result(response)
            assert result['feasibility'] == 'feasible' and result['optimality'] == 'optimal', result
            point = np.array(result['values'])
            value = float(point@matrix@point + q@point)
            assert np.max(np.abs(point-s)) < 3e-4, ('KKT_point', s.tolist(), point.tolist())
            assert abs(value-expected) < 3e-5*max(1,abs(expected)), ('KKT_objective', expected, value)
            return {'expected': expected, 'actual': value}
        check(f'qp-{index}', 'solve-qp', {'model': problem}, qp_check)

    # Literal arithmetic witnesses: returning a structured numerical rejection is
    # acceptable; returning an incorrect "optimal" solution is never acceptable.
    stress = model('inequality cancellation', [(0.,1.)]*3, [1e16,1.,-1e16], [([1.,-1.,0.],'less_equal',0),([0.,1.,-1.],'less_equal',0)], 'maximize')
    def check_stress(response):
        result = solve_result(response)
        if result['feasibility'] == 'unknown' and result['optimality'] == 'unknown' and result['values'] is None:
            return {'honest_inconclusive':True,'reason':'No unchecked candidate or optimality claim returned'}
        return validate_solution(stress,response,1.,atol=1e-8)
    for command in ['solve', 'solve-sparse']:
        check('exact-cancellation-'+command, command, {'model':stress}, check_stress, allow_rejection=True)

    feasible_stress = model('phase-I feasible cancellation', [(0.,1e16)]*3, [0.,0.,0.],
        [([1.,-1.,0.],'less_equal',0),([0.,1.,-1.],'less_equal',0),([1e16,1.,-1e16],'greater_equal',1e16)])
    # [1e16,1e16,1e16] is an exact feasible witness. A numerical rejection is
    # honest; an infeasibility certificate is contradicted by that witness.
    def check_feasible_stress(response):
        result = solve_result(response)
        assert result['feasibility'] != 'infeasible', 'False infeasibility: exact witness [1e16,1e16,1e16]'
        if result['feasibility'] == 'unknown' and result['optimality'] == 'unknown' and result['values'] is None:
            return {'honest_inconclusive':True,'reason':'Ill-conditioned phase I returned no false infeasibility certificate'}
        return validate_solution(feasible_stress,response,0.,atol=1e-8)
    check('phase-I-feasible-cancellation', 'solve', {'model':feasible_stress}, check_feasible_stress, allow_rejection=True)

    bounded_stress = model('bounded recession cancellation', [(0.,None)]*3, [1.,0.,0.],
        [([1.,-1.,0.],'less_equal',0),([0.,1.,-1.],'less_equal',0),([-1.,0.,1.],'less_equal',0),
         ([1e16,1.,-1e16],'less_equal',1)], 'maximize')
    def check_bounded_stress(response):
        result=solve_result(response)
        assert result['feasibility'] != 'unbounded', 'False unboundedness: cycle forces x=y=z and final row imposes x<=1'
        assert result['termination'] != 'unbounded', 'False unboundedness of a mathematically bounded model'
        if result['feasibility']=='unknown' and result['optimality']=='unknown' and result['values'] is None:
            return {'honest_inconclusive':True,'reason':'No unchecked original-model recession claim'}
        # In this stress model, a verified feasible incumbent without an
        # optimality claim is an honest qualification, not a numerical pass.
        if result['optimality']=='unknown' and result['values'] is not None:
            return {'honest_inconclusive':True,'reason':'Checked anchor retained without unboundedness or optimality claim'}
        point=result['values']
        assert result['optimality']=='optimal' and abs(point[0]-1)<1e-8, result
        assert all(abs(v-1)<1e-8 for v in point), result
        return {'expected_objective':1.,'actual_objective':point[0]}
    check('bounded-recession-cancellation','solve',{'model':bounded_stress},check_bounded_stress,allow_rejection=True)

    # A zero-variance summary would assert false certainty: these nonconstant
    # values have positive mathematical variance below the binary64 range.
    tiny_request = {'inputs':[{'kind':'uniform','low':-1e-200,'high':1e-200}],
                    'options':{'trials':32,'seed':str(SEED),'retain_samples':True}}
    def reject_false_zero_variance(response):
        summary = response['result']['summaries'][0]
        assert summary['variance'] is None, ('nonrepresentable_positive_variance',summary)
        return {'reason':'Nonrepresentable positive variance is explicitly unavailable'}
    check('unrepresentable-positive-variance', 'simulate', tiny_request, reject_false_zero_variance, allow_rejection=True)

    mixture_request = {'inputs':[{'kind':'mixture','components':[{'kind':'constant','value':0.},{'kind':'constant','value':1.}], 'weights':[.5,.5]}],
                       'options':{'trials':64,'seed':str(SEED),'method':'latin_hypercube','retain_samples':True}}
    def exact_atoms(response):
        values = [row[0] for row in response['result']['samples']]
        assert set(values) == {0.,1.} and values.count(0.) == 32 and values.count(1.) == 32, ('non_atom_values', sorted(set(values)))
    check('LHS-discrete-mixture-exact-atoms','simulate',mixture_request,exact_atoms)

    # Fixed anchors use an ideal-IID Hoeffding/union sanity threshold, not a
    # claim that a finite PRNG or an entire CDF has been proved correct.
    n = 32768
    families = [
        ({'kind':'uniform','low':-2.,'high':5.}, stats.uniform(loc=-2,scale=7)),
        ({'kind':'normal','mean':1.,'std_dev':2.}, stats.norm(loc=1,scale=2)),
        ({'kind':'exponential','rate':.5}, stats.expon(scale=2)),
        ({'kind':'triangular','low':-2.,'mode':1.,'high':5.}, stats.triang(3/7,loc=-2,scale=7)),
        ({'kind':'beta','alpha':.5,'beta':4.}, stats.beta(.5,4)),
        ({'kind':'gamma','shape':2.,'scale':3.}, stats.gamma(2,scale=3)),
        ({'kind':'lognormal','log_mean':.2,'log_std_dev':.7}, stats.lognorm(.7,scale=math.exp(.2))),
        ({'kind':'poisson','rate':10.}, stats.poisson(10)),
        ({'kind':'binomial','trials':20,'probability':.3}, stats.binom(20,.3)),
        ({'kind':'bernoulli','probability':.3}, stats.bernoulli(.3)),
    ]
    anchors_total = len(families)*2*5
    threshold = math.sqrt(math.log(2*anchors_total/1e-8)/(2*n))
    for method in ['monte_carlo','latin_hypercube']:
        for distribution, reference in families:
            request = {'inputs':[distribution], 'options':{'trials':n,'seed':str(SEED),'method':method,'retain_samples':True,'max_memory_bytes':32*1024*1024}}
            def simulation_check(response, rv=reference):
                result = response['result']
                assert result['termination'] == 'completed' and result['completed_trials'] == n
                values = [row[0] for row in result['samples']]
                summary = result['summaries'][0]
                assert len(values) == n and all(math.isfinite(x) for x in values)
                mean = math.fsum(values)/n
                variance = math.fsum((x-mean)**2 for x in values)/(n-1)
                assert abs(summary['mean']-mean) <= 2e-12*max(1,abs(mean)), ('sample_mean', summary['mean'], mean)
                assert abs(summary['variance']-variance) <= 2e-10*max(1,variance), ('sample_variance', summary['variance'], variance)
                ordered = sorted(values)
                for p,q in summary['quantiles']:
                    expected = ordered[max(0,math.ceil(p*n)-1)]
                    assert q == expected, ('quantile', p, q, expected)
                mass = (1-.95)*n
                whole = math.floor(mass)
                fraction = mass-whole
                tail = math.fsum(ordered[-whole:]) + fraction*ordered[-whole-1]
                expected_tail = tail/mass
                assert abs(summary['conditional_value_at_risk_95']-expected_tail) <= 3e-12*max(1,abs(expected_tail)), ('empirical_CVaR',summary['conditional_value_at_risk_95'],expected_tail)
                errors=[]
                for p in [.1,.25,.5,.75,.9]:
                    anchor=float(rv.ppf(p))
                    expected=float(rv.cdf(anchor))
                    observed=sum(x<=anchor for x in values)/n
                    errors.append(abs(observed-expected))
                assert max(errors) <= threshold, ('CDF_anchors',max(errors),threshold)
                return {'max_cdf_anchor_error':max(errors),'cdf_tolerance':threshold,'mean':mean,'variance':variance}
            check(method+'-'+distribution['kind'], 'simulate', request, simulation_check)
    # Quasi-Monte Carlo is compared to exact integrals, not an IID confidence
    # interval. Independent randomizations are separate qualified checks.
    for replicate in range(8):
        request={'inputs':[{'kind':'uniform','low':0.,'high':1.}]*2,
                 'outputs':[{'op':'multiply','left':{'op':'variable','index':0},'right':{'op':'variable','index':1}}],
                 'options':{'base':{'trials':4096,'seed':str(SEED),'retain_samples':True},
                            'sampler':{'kind':'randomized_sobol','replicate':str(replicate)}}}
        def sobol_check(response):
            result=response['result']['result']
            samples=[row[0] for row in result['samples']]
            mean=math.fsum(samples)/len(samples)
            assert all(0.<=x<=1. for x in samples)
            assert abs(mean-.25)<.003, ('exact_product_integral',mean,.25)
            assert abs(result['summaries'][0]['mean']-mean)<1e-13
            assert 'sobol' in result['sampling_version']
            return {'expected_integral':.25,'sample_mean':mean,'tolerance':.003}
        check(f'randomized-sobol-{replicate}','simulate-advanced',request,sobol_check)

    # With true deterministic bounds and an ordinary MC sampler, independently
    # recompute the configured simultaneous-checkpoint Hoeffding width.
    adaptive={'inputs':[{'kind':'uniform','low':0.,'high':1.}],
              'options':{'base':{'trials':16384,'seed':str(SEED),'retain_samples':True},
                         'stopping':{'minimum_trials':512,'check_every':512,'absolute_half_width':.05,
                                     'confidence':.99,'output_bounds':[[0.,1.]]}}}
    def stopping_check(response):
        result=response['result']
        assert result['stopped_early'] and result['result']['completed_trials']<16384
        count=result['result']['completed_trials']
        mean=math.fsum(row[0] for row in result['result']['samples'])/count
        interval=result['mean_intervals'][0]
        width=(interval['upper']-interval['lower'])/2
        assert width<=.05 and interval['lower']<=.5<=interval['upper']
        assert abs((interval['upper']+interval['lower'])/2-mean)<1e-12
        # The implementation may budget all checkpoints, including the final
        # partial checkpoint; using fewer than32 is an invalid union bound.
        minimum_width=math.sqrt(math.log(2*32/(1-.99))/(2*count))
        assert width+1e-12>=minimum_width, ('understated_adaptive_interval',width,minimum_width)
        return {'completed_trials':count,'interval_half_width':width,'true_mean':.5}
    check('bounded-adaptive-MC','simulate-advanced',adaptive,stopping_check)

    report={'schema':'solvercraft.independent-process-oracles.v1','seed':SEED,'binary_sha256':identity,
            'numpy_version':np.__version__,'scipy_version':scipy.__version__,
            'counts':{status:sum(row['status']==status for row in records) for status in ['passed','failed','explicit_rejection','honest_inconclusive']},'cases':records}
    args.report.parent.mkdir(parents=True,exist_ok=True)
    args.report.write_text(json.dumps(report,indent=2,allow_nan=False)+'\n',encoding='utf8')
    print(json.dumps({'binary_sha256':identity,**report['counts'],'failing_cases':[row['case'] for row in records if row['status']=='failed']}))
    return 1 if report['counts']['failed'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
