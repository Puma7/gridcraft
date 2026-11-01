"""Independent, test-only SciPy oracle. No product dependency or network runtime.

Run with SciPy1.18.1/NumPy2.5.4 on PYTHONPATH. Both packages are BSD-3-Clause;
their wheel licenses remain in the isolated test environment, not the product.
Parameter mappings follow https://docs.scipy.org/doc/scipy/reference/stats.html
and each scipy.stats.<family> primary reference (not solvercraft roundtrips).
"""
from __future__ import annotations
import argparse, hashlib, json, math, os, pathlib, platform, subprocess, sys, tempfile, warnings
import numpy as np
import scipy
from scipy import stats

PROBE = r'''
use std::io::{self,BufRead};
use solvercraft_math::{PreparedDistribution,density::log_density};
use solvercraft_types::{Distribution,Result};
use serde_json::{Value,json};
fn answer(value:Result<f64>)->Value{match value{Ok(x) if x.is_finite()=>json!({"value":x}),Ok(x)=>json!({"nonfinite":if x.is_nan(){"nan"}else if x.is_sign_negative(){"-inf"}else{"inf"}}),Err(e)=>json!({"error":e})}}
fn run(v:Value)->Value{
 let distribution:Distribution=match serde_json::from_value(v["distribution"].clone()){Ok(d)=>d,Err(e)=>return json!({"parse_error":e.to_string()})};
 let prepared=match PreparedDistribution::new(&distribution){Ok(d)=>d,Err(e)=>return json!({"prepare_error":e})};
 let p=v["probabilities"].as_array().into_iter().flatten().map(|p|answer(prepared.quantile(p.as_f64().unwrap_or(0.0)))).collect::<Vec<_>>();
 let xs=v["xs"].as_array().into_iter().flatten().filter_map(Value::as_f64).collect::<Vec<_>>();
 json!({"input_xs":xs,"input_probabilities":v["probabilities"],"quantiles":p,"cdfs":xs.iter().map(|x|answer(prepared.cdf(*x))).collect::<Vec<_>>(),"logdensities":xs.iter().map(|x|answer(log_density(&distribution,*x))).collect::<Vec<_>>()})
}
fn main(){for line in io::stdin().lock().lines(){match line{Ok(line)=>{let value=serde_json::from_str(&line).map(run).unwrap_or_else(|e|json!({"parse_error":e.to_string()}));println!("{value}");},Err(_)=>break,}}}
'''

def cases():
    rows=[]
    def add(d, family, *args, label=None, extra=(), **kw):
        rows.append((label or json.dumps(d,sort_keys=True),d,getattr(stats,family)(*args,**kw),extra))
    for shape in [.1,.5,1.,2.,10.,100.,1000.,10000.]:
        add(dict(kind='gamma',shape=shape,scale=2.),'gamma',shape,scale=2.)
        add(dict(kind='beta',alpha=shape,beta=shape),'beta',shape,shape)
        add(dict(kind='inverse_gamma',shape=shape,scale=2.),'invgamma',shape,scale=2.)
    for a,b in [(.1,2.),(2.,.1),(.5,5.),(5.,.5),(100.,1000.)]:
        add(dict(kind='beta',alpha=a,beta=b),'beta',a,b)
    for shape in [.1,.5,1.,2.,10.]:
        add(dict(kind='weibull',shape=shape,scale=2.),'weibull_min',shape,scale=2.)
        add(dict(kind='log_logistic',shape=shape,scale=2.),'fisk',shape,scale=2.)
        add(dict(kind='pareto',shape=shape,minimum=2.),'pareto',shape,scale=2.)
        add(dict(kind='pareto2',shape=shape,scale=2.),'lomax',shape,scale=2.)
        add(dict(kind='frechet',shape=shape,scale=2.),'invweibull',shape,scale=2.)
        add(dict(kind='fatigue_life',shape=shape,scale=2.),'fatiguelife',shape,scale=2.)
    for df in [.1,.5,1.,2.,10.,100.,1000.]:
        add(dict(kind='student_t',degrees_of_freedom=df,location=1.,scale=2.),'t',df,loc=1.,scale=2.)
        add(dict(kind='chi_squared',degrees_of_freedom=df),'chi2',df)
        add(dict(kind='f',numerator_df=df,denominator_df=3.),'f',df,3.)
    for shape in [-100.,-5.,0.,5.,100.]:
        add(dict(kind='skew_normal',location=0.,scale=1.,shape=shape),'skewnorm',shape,extra=(-10.,10.))
    for shape in [.01,1.,1000.]:
        add(dict(kind='inverse_gaussian',mean=2.,shape=shape),'invgauss',2./shape,scale=shape)
    for gamma,delta in [(-2.,.5),(0.,1.),(2.,3.)]:
        for suffix in ['su','sb']:
            add(dict(kind='johnson_'+suffix,gamma=gamma,delta=delta,location=1.,scale=2.),'johnson'+suffix,gamma,delta,loc=1.,scale=2.)
        add(dict(kind='johnson_sl',gamma=gamma,delta=delta,location=1.,scale=2.),'lognorm',1./delta,loc=1.,scale=2.*math.exp(-gamma/delta))
    for c,k in [(.2,.5),(1.,1.),(2.,3.),(10.,.2)]:
        add(dict(kind='burr12',shape_c=c,shape_k=k,scale=2.),'burr12',c,k,scale=2.)
        add(dict(kind='dagum',shape=c,power=k,scale=2.),'burr',c,k,scale=2.)
        add(dict(kind='beta_prime',alpha=c,beta=k,scale=2.),'betaprime',c,k,scale=2.)
    for name in ['normal','cauchy','logistic','laplace','gumbel','min_extreme','hyperbolic_secant']:
        family={'normal':'norm','gumbel':'gumbel_r','min_extreme':'gumbel_l','hyperbolic_secant':'hypsecant'}.get(name,name)
        d=dict(kind=name,location=1.,scale=2.) if name!='normal' else dict(kind=name,mean=1.,std_dev=2.)
        add(d,family,loc=1.,scale=2.,extra=(1e200,) if name=='cauchy' else ())
    add(dict(kind='lognormal',log_mean=1.,log_std_dev=.5),'lognorm',.5,scale=math.exp(1.))
    add(dict(kind='uniform',low=-2.,high=5.),'uniform',loc=-2.,scale=7.)
    add(dict(kind='triangular',low=-2.,mode=1.,high=5.),'triang',3/7,loc=-2.,scale=7.)
    add(dict(kind='beta_pert',low=-2.,mode=1.,high=5.,weight=4.),'beta',1+4*3/7,1+4*4/7,loc=-2.,scale=7.)
    add(dict(kind='rayleigh',scale=2.),'rayleigh',scale=2.)
    add(dict(kind='levy',location=1.,scale=2.),'levy',loc=1.,scale=2.)
    add(dict(kind='reciprocal',low=.1,high=100.),'reciprocal',.1,100.)
    add(dict(kind='exponential',rate=.5),'expon',scale=2.)
    add(dict(kind='erlang',shape=7,scale=2.),'gamma',7,scale=2.)
    for p in [.01,.3,.99]:
        add(dict(kind='bernoulli',probability=p),'bernoulli',p)
        add(dict(kind='geometric',probability=p),'nbinom',1,p)
        add(dict(kind='negative_binomial',successes=5,probability=p),'nbinom',5,p)
        add(dict(kind='binomial',trials=100,probability=p),'binom',100,p)
        add(dict(kind='logarithmic',probability=p),'logser',p)
    for rate in [.001,10.,512.]:add(dict(kind='poisson',rate=rate),'poisson',rate)
    add(dict(kind='discrete_uniform',low=-3,high=7),'randint',-3,8)
    add(dict(kind='hypergeometric',population=1000,successes=300,draws=25),'hypergeom',1000,300,25)
    rows.append(('empirical repeated knots',dict(kind='empirical',values=[1.,2.,2.,4.]),stats.rv_discrete(name='empirical_reference',values=([1.,2.,4.],[.25,.5,.25])),()))
    rows.append(('categorical reordered values',dict(kind='categorical',values=[4.,1.,2.],weights=[1.,1.,2.]),stats.rv_discrete(name='categorical_reference',values=([1.,2.,4.],[.25,.5,.25])),()))
    add(dict(kind='kumaraswamy',alpha=1.,beta=2.),'beta',1.,2.)
    add(dict(kind='kumaraswamy',alpha=2.,beta=1.),'powerlaw',2.)
    add(dict(kind='double_triangular',low=0.,mode=1.,high=4.,left_probability=.25),'triang',.25,loc=0.,scale=4.)
    return rows

def finite_or_string(value):
    value=float(value)
    return value if math.isfinite(value) else str(value)

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--root',type=pathlib.Path,default=pathlib.Path(__file__).resolve().parents[2]);ap.add_argument('--scratch',type=pathlib.Path);ap.add_argument('--report',type=pathlib.Path);args=ap.parse_args()
    root=args.root.resolve();scratch=(args.scratch or pathlib.Path(tempfile.mkdtemp(prefix='solvercraft-oracle-'))).resolve();(scratch/'src').mkdir(parents=True,exist_ok=True)
    def source_hashes():
        paths=list((root/'crates'/'math'/'src').glob('*.rs'))+[root/'crates'/'types'/'src'/'lib.rs']
        return {str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}
    before=source_hashes()
    # This isolated test probe reuses only already pinned/cached Rust dependencies.
    manifest='[workspace]\n[package]\nname="distribution-oracle-probe"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nserde_json={version="=1.0.149",features=["float_roundtrip"]}\n'
    manifest+='solvercraft-math={path='+json.dumps(str(root/'crates'/'math'))+'}\nsolvercraft-types={path='+json.dumps(str(root/'crates'/'types'))+'}\n'
    (scratch/'Cargo.toml').write_text(manifest,encoding='utf-8');(scratch/'src'/'main.rs').write_text(PROBE,encoding='utf-8')
    env=os.environ.copy();env.update(CARGO_TARGET_DIR=str(scratch/'target'),CARGO_INCREMENTAL='0')
    subprocess.run([env.get('CARGO','cargo'),'build','--manifest-path',str(scratch/'Cargo.toml'),'--offline','--quiet'],env=env,check=True)
    ps=[1e-10,1e-7,.001,.1,.5,.9,.999,1-1e-7,1-1e-10];rows=cases();inputs=[];references=[];oracle_warnings=[]
    for label,d,rv,extra in rows:
        qs=[];warned=[]
        for p in ps:
            with warnings.catch_warnings(record=True) as captured:
                warnings.simplefilter('always');qs.append(float(rv.ppf(p)))
            warned.append(bool(captured))
            if captured:oracle_warnings.append(dict(case=label,p=p,categories=[m.category.__name__ for m in captured],note='Oracle emitted a convergence/numerical warning; this quantile is excluded rather than treated as authoritative.'))
        qs=np.asarray(qs);xs=[float(x) for x in qs if math.isfinite(x)]+list(extra)
        inputs.append(dict(distribution=d,probabilities=ps,xs=xs));references.append((label,d,rv,qs,xs,warned))
    executable='distribution-oracle-probe.exe' if os.name=='nt' else 'distribution-oracle-probe'
    run=subprocess.run([str(scratch/'target'/'debug'/executable)],input=''.join(json.dumps(v,allow_nan=False)+'\n' for v in inputs),text=True,capture_output=True,env=env,check=True)
    outputs=[json.loads(v) for v in run.stdout.splitlines()];assert len(outputs)==len(inputs)
    failures=[];counts=dict(distributions=len(rows),quantiles=0,cdfs=0,logdensities=0,explicit_range_errors=0,oracle_endpoint_or_unavailable_quantiles=0,transport_float_changes=0);max_errors=dict(quantile_relative=0.,quantile_zero_absolute=0.,cdf_absolute=0.,logdensity_absolute=0.)
    for (label,d,rv,qs,xs,warned),out in zip(references,outputs):
        if 'prepare_error' in out or 'parse_error' in out:failures.append(dict(case=label,operation='prepare',actual=out));continue
        parsed_ps=out['input_probabilities'];parsed_xs=out['input_xs'];counts['transport_float_changes']+=sum(a!=b for a,b in zip(ps,parsed_ps))+sum(a!=b for a,b in zip(xs,parsed_xs))
        for p,expected,actual,oracle_warned in zip(parsed_ps,qs,out['quantiles'],warned):
            if not math.isfinite(expected) or oracle_warned:
                counts['oracle_endpoint_or_unavailable_quantiles']+=1
                if 'error' in actual:counts['explicit_range_errors']+=1
                continue
            counts['quantiles']+=1
            if 'value' not in actual:failures.append(dict(case=label,operation='quantile',p=p,expected=finite_or_string(expected),actual=actual));continue
            error=abs(actual['value']-expected)/max(abs(expected),1e-100) if expected else abs(actual['value']);metric='quantile_relative' if expected else 'quantile_zero_absolute';max_errors[metric]=max(max_errors[metric],error)
            if error>(2e-6 if expected else 1e-10):failures.append(dict(case=label,operation='quantile',p=p,expected=expected,actual=actual['value'],error_measure=metric,error=error))
        for x,actual_cdf,actual_log in zip(parsed_xs,out['cdfs'],out['logdensities']):
            expected=float(rv.cdf(x));counts['cdfs']+=1
            if 'value' not in actual_cdf:failures.append(dict(case=label,operation='cdf',x=x,expected=finite_or_string(expected),actual=actual_cdf))
            else:
                error=abs(actual_cdf['value']-expected);max_errors['cdf_absolute']=max(max_errors['cdf_absolute'],error);tol=5e-7 if d['kind'] in ('normal','lognormal') else 2e-8
                if error>tol:failures.append(dict(case=label,operation='cdf',x=x,expected=expected,actual=actual_cdf['value'],absolute_error=error))
            expected_log=float(rv.logpmf(x) if hasattr(rv,'logpmf') else rv.logpdf(x));counts['logdensities']+=1
            if not math.isfinite(expected_log):continue
            if 'value' not in actual_log:failures.append(dict(case=label,operation='logdensity',x=x,expected=expected_log,actual=actual_log));continue
            error=abs(actual_log['value']-expected_log);max_errors['logdensity_absolute']=max(max_errors['logdensity_absolute'],error)
            if error>2e-6+1e-10*abs(expected_log):failures.append(dict(case=label,operation='logdensity',x=x,expected=expected_log,actual=actual_log['value'],absolute_error=error))
    after=source_hashes()
    parameterizations=[dict(case=label,distribution=d,scipy_family=getattr(getattr(rv,'dist',rv),'name','custom'),scipy_args=list(getattr(rv,'args',())),scipy_keywords=getattr(rv,'kwds',{})) for label,d,rv,_ in rows]
    report=dict(python=sys.version,scipy=scipy.__version__,numpy=np.__version__,platform=platform.platform(),license_note='SciPy and NumPy BSD-3-Clause; isolated test-only environment, not product dependencies.',sources=['https://docs.scipy.org/doc/scipy/reference/stats.html','https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.invgauss.html','https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.burr.html'],probabilities=ps,thresholds=dict(quantile_relative=2e-6,quantile_zero_absolute=1e-10,cdf_absolute=2e-8,legacy_normal_lognormal_cdf_absolute=5e-7,logdensity_absolute=2e-6,logdensity_additional_relative=1e-10),parameterizations=parameterizations,oracle_warnings=oracle_warnings,counts=counts,max_errors=max_errors,failure_count=len(failures),failures=failures,source_stable=before==after,source_sha256=before,source_after_sha256=after,harness_sha256=hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest())
    destination=args.report or root/'docs'/'qa'/'distribution-oracle-report.json';destination.parent.mkdir(parents=True,exist_ok=True);destination.write_text(json.dumps(report,indent=2,allow_nan=False)+'\n',encoding='utf-8')
    print(json.dumps({k:report[k] for k in ('scipy','numpy','source_stable','counts','max_errors','failure_count')},indent=2));print('First failures:',json.dumps(failures[:10],indent=2,allow_nan=False));return int(bool(failures) or not report['source_stable'] or bool(counts['transport_float_changes']))
if __name__=='__main__':raise SystemExit(main())
