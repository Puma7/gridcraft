"""Large deterministic, independent SciPy distribution matrix (test-only).

Parameter conventions follow primary scipy.stats documentation, especially
truncnorm's standardized cutoffs and invgauss(mu=mean/shape,scale=shape).
Documented legacy absolute-CDF contracts and stronger tail-accuracy targets
are reported separately. Unsupported/range errors are never counted as passes.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import warnings
from decimal import Decimal, localcontext

import numpy as np
import scipy
from scipy import stats,integrate,special
import distribution_oracle as original

PS=[1e-20,1e-12,1e-8,1e-5,.001,.01,.1,.25,.5,.75,.9,.99,.999,1-1e-8,1-1e-12]
SHAPES=[.001,.03,.2,.7,1.,2.5,13.,123.,1000.]

def qualified_quantile(distribution,p,scipy_value):
    """Independent 90-digit elementary inverses repair reference conditioning.

    SciPy PPF itself can cancel at 1-p or tan(pi/4). These formulas follow
    the mathematical CDFs, not any Solvercraft implementation or its output.
    """
    kind=distribution['kind']
    symmetric={'normal':'mean','student_t':'location','cauchy':'location','logistic':'location','laplace':'location','hyperbolic_secant':'location'}
    if p==.5 and kind in symmetric:return distribution[symmetric[kind]],'exact_symmetric_median'
    if p==.5 and kind=='shift_scale' and distribution['base']['kind'] in symmetric:
        return distribution['shift']+distribution['scale']*distribution['base'][symmetric[distribution['base']['kind']]],'exact_symmetric_median'
    elementary={'log_logistic','pareto','pareto2','frechet','weibull','burr12','dagum','exponential'}
    if kind not in elementary:return scipy_value,'scipy'
    with localcontext() as context:
        context.prec=90
        one=Decimal(1);q=Decimal.from_float(p)
        get=lambda name:Decimal.from_float(float(distribution[name]))
        if kind=='exponential':value=-(one-q).ln()/get('rate')
        elif kind=='dagum':value=get('scale')*(((-q.ln()/get('power')).exp()-one).ln()/(-get('shape'))).exp()
        elif kind=='burr12':value=get('scale')*(((-(one-q).ln()/get('shape_k')).exp()-one).ln()/get('shape_c')).exp()
        elif kind=='log_logistic':value=get('scale')*((q.ln()-(one-q).ln())/get('shape')).exp()
        elif kind=='pareto':value=get('minimum')*(-(one-q).ln()/get('shape')).exp()
        elif kind=='pareto2':value=get('scale')*((-(one-q).ln()/get('shape')).exp()-one)
        elif kind=='frechet':value=get('scale')*(-(-q.ln()).ln()/get('shape')).exp()
        else:value=get('scale')*((-(one-q).ln()).ln()/get('shape')).exp()
        return float(value),'decimal90_closed_form'

def normal_interval_mass(low,high):
    if high<=low:return 0.
    width=high-low
    # Integrate a positive rescaled density on [0,1], retaining short intervals
    # without subtracting nearly equal rounded CDF or survival values.
    value,error=integrate.quad(lambda t:math.exp(-low*width*t-.5*(width*t)**2),0.,1.,epsabs=1e-13,epsrel=1e-13)
    if error>1e-10*value:raise RuntimeError('Independent normal interval quadrature did not converge')
    return width*math.exp(-.5*low*low)/math.sqrt(2*math.pi)*value

def qualified_cdf(distribution,x,scipy_value):
    kind=distribution['kind']
    if kind=='pareto':
        if x<=distribution['minimum']:return 0.,True
        with localcontext() as context:
            context.prec=90
            ratio=Decimal.from_float(distribution['minimum'])/Decimal.from_float(x)
            value=Decimal(1)-(Decimal.from_float(distribution['shape'])*ratio.ln()).exp()
            return float(value),True
    if kind=='skew_normal' and distribution['shape']>0:
        z=(x-distribution['location'])/distribution['scale']
        if z<0:
            a=distribution['shape'];start=-z
            def log_integrand(u):return math.log(2)-.5*u*u-.5*math.log(2*math.pi)+float(special.log_ndtr(-a*u))
            anchor=log_integrand(start)
            upper_log=math.log(2)+float(special.log_ndtr(-start))+float(special.log_ndtr(-a*start))
            if upper_log<-747:return 0.,True
            ratio=math.exp(-.5*(a*start)**2-.5*math.log(2*math.pi)-float(special.log_ndtr(-a*start)))
            width=1/(start+a*ratio)
            scaled,error=integrate.quad(lambda v:math.exp(log_integrand(start+width*v)-anchor),0,np.inf,epsabs=1e-13,epsrel=1e-12,limit=160)
            if error>1e-9*scaled:raise RuntimeError('Independent skew-normal tail quadrature did not converge')
            return math.exp(anchor)*scaled*width,True
    if kind=='truncated' and distribution['base']==dict(kind='normal',mean=0.,std_dev=1.):
        low=distribution['lower'];high=distribution['upper']
        if x<=low:return 0.,True
        if x>=high:return 1.,True
        mass=normal_interval_mass(low,high)
        if x-low<high-x:return normal_interval_mass(low,x)/mass,True
        return 1-normal_interval_mass(x,high)/mass,True
    return scipy_value,False

def clean(value):
    if isinstance(value,dict):return {str(k):clean(v) for k,v in value.items()}
    if isinstance(value,(list,tuple)):return [clean(v) for v in value]
    if isinstance(value,(float,np.floating)) and not math.isfinite(value):return 'nan' if math.isnan(value) else '-inf' if value<0 else 'inf'
    if isinstance(value,np.integer):return int(value)
    return value

def cases():
    rows=[]
    seen=set()
    def add(d,family,*args,label=None,extra=(),**kw):
        key=json.dumps(d,sort_keys=True)
        if key in seen:return
        seen.add(key)
        rv=getattr(stats,family)(*args,**kw)
        rows.append((label or key,d,rv,list(extra),family))
    for label,d,rv,extra in original.cases():
        key=json.dumps(d,sort_keys=True)
        if key not in seen:
            seen.add(key);rows.append((label,d,rv,list(extra),getattr(getattr(rv,'dist',rv),'name','custom')))
    for a in SHAPES:
        for scale in [1e-50,.3,7.,1e50]:
            for kind,ref in [('gamma','gamma'),('inverse_gamma','invgamma')]:add(dict(kind=kind,shape=a,scale=scale),ref,a,scale=scale)
        for b in SHAPES:add(dict(kind='beta',alpha=a,beta=b),'beta',a,b)
        add(dict(kind='chi_squared',degrees_of_freedom=a),'chi2',a)
        for scale in [.01,1.,100.]:
            add(dict(kind='student_t',degrees_of_freedom=a,location=0.,scale=scale),'t',a,scale=scale)
        for b in [.03,.5,2.,31.,1000.]:add(dict(kind='f',numerator_df=a,denominator_df=b),'f',a,b)
    for shape in [.01,.1,.5,1.,3.,17.,100.]:
        for scale in [1e-50,1.,1e50]:
            for kind,ref in [('weibull','weibull_min'),('log_logistic','fisk'),('pareto','pareto'),('pareto2','lomax'),('frechet','invweibull'),('fatigue_life','fatiguelife')]:
                d=dict(kind=kind,shape=shape,**({'minimum':scale} if kind=='pareto' else {'scale':scale}))
                add(d,ref,shape,scale=scale)
    for kind,ref in [('normal','norm'),('cauchy','cauchy'),('logistic','logistic'),('laplace','laplace'),('gumbel','gumbel_r'),('min_extreme','gumbel_l'),('hyperbolic_secant','hypsecant')]:
        for loc in [-3.,0.,2.]:
            for scale in [1e-50,.001,1.,1e4,1e50]:
                d=dict(kind=kind,**({'mean':loc,'std_dev':scale} if kind=='normal' else {'location':loc,'scale':scale}))
                add(d,ref,loc=loc,scale=scale)
    for shape in [-100.,-10.,-2.,-.1,0.,.1,2.,10.,100.]:
        for loc in [-2.,0.,3.]:
            for scale in [.1,1.,10.]:add(dict(kind='skew_normal',shape=shape,location=loc,scale=scale),'skewnorm',shape,loc=loc,scale=scale)
    for mean in [.001,.3,1.,7.,100.]:
        for shape in [.001,.2,2.,100.,10000.]:add(dict(kind='inverse_gaussian',mean=mean,shape=shape),'invgauss',mean/shape,scale=shape)
    for gamma in [-5.,-1.,0.,2.,6.]:
        for delta in [.1,.5,1.,3.,10.]:
            for scale in [.3,3.]:
                for suffix in ['su','sb']:
                    add(dict(kind='johnson_'+suffix,gamma=gamma,delta=delta,location=1.,scale=scale),'johnson'+suffix,gamma,delta,loc=1.,scale=scale)
                add(dict(kind='johnson_sl',gamma=gamma,delta=delta,location=1.,scale=scale),'lognorm',1/delta,loc=1.,scale=scale*math.exp(-gamma/delta))
    for a in [.05,.2,.8,1.,3.,30.]:
        for b in [.05,.2,.8,1.,3.,30.]:
            for kind,ref,fields in [('burr12','burr12',dict(shape_c=a,shape_k=b)),('dagum','burr',dict(shape=a,power=b)),('beta_prime','betaprime',dict(alpha=a,beta=b))]:
                add(dict(kind=kind,scale=.7,**fields),ref,a,b,scale=.7)
    for mu in [-8.,-1.,0.,3.,10.]:
        for sd in [.1,.5,1.,3.,10.]:add(dict(kind='lognormal',log_mean=mu,log_std_dev=sd),'lognorm',sd,scale=math.exp(mu))
    for rate in [.001,.1,1.,3.,10.,31.,100.,256.,512.]:add(dict(kind='poisson',rate=rate),'poisson',rate)
    for n in [1,2,10,50,1000,10000]:
        for p in [1e-7,.001,.1,.5,.99,1-1e-7]:add(dict(kind='binomial',trials=n,probability=p),'binom',n,p)
    for n in [1,2,7,100,1000]:
        for p in [.01,.2,.5,.95,.999]:add(dict(kind='negative_binomial',successes=n,probability=p),'nbinom',n,p)
    for p in [.0001,.01,.2,.5,.99,1.]:add(dict(kind='geometric',probability=p),'nbinom',1,p)
    for n in [10,100,1000]:
        for successes in [n//10,n//2,n]:
            for draws in [min(3,n),n//2]:add(dict(kind='hypergeometric',population=n,successes=successes,draws=draws),'hypergeom',n,successes,draws)
    for p in [.001,.1,.5,.9,.99]:add(dict(kind='logarithmic',probability=p),'logser',p)
    for mode in [0.,.01,.3,.99,1.]:
        for width in [.01,3.,100.]:add(dict(kind='triangular',low=-2.,mode=-2+mode*width,high=-2+width),'triang',mode,loc=-2,scale=width)
    for weight in [.1,1.,4.,20.,100.]:
        for mode in [0.,.1,.5,.9,1.]:add(dict(kind='beta_pert',low=-2.,mode=-2+7*mode,high=5.,weight=weight),'beta',1+weight*mode,1+weight*(1-mode),loc=-2,scale=7)
    for loc in [-2.,0.,3.]:
        for scale in [-10.,-1.,-.1,.1,1.,10.]:
            for kind,ref in [('normal','norm'),('cauchy','cauchy')]:
                base=dict(kind=kind,**({'mean':0.,'std_dev':1.} if kind=='normal' else {'location':0.,'scale':1.}))
                add(dict(kind='shift_scale',base=base,shift=loc,scale=scale),ref,loc=loc,scale=abs(scale))
    for low,high in [(-4.,2.),(-1.,1.),(4.,5.),(5.,6.),(-6.,-5.)]:
        add(dict(kind='truncated',base=dict(kind='normal',mean=0.,std_dev=1.),lower=low,upper=high),'truncnorm',low,high)
    return rows

def build_probe(sdk,scratch):
    (scratch/'src').mkdir(parents=True,exist_ok=True)
    manifest='[workspace]\n[package]\nname="systematic-distribution-probe"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nserde_json={version="=1.0.149",features=["float_roundtrip"]}\n'
    manifest+='solvercraft-math={path='+json.dumps(str(sdk/'crates/math'))+'}\nsolvercraft-types={path='+json.dumps(str(sdk/'crates/types'))+'}\n'
    (scratch/'Cargo.toml').write_text(manifest,encoding='utf8')
    (scratch/'src/main.rs').write_text(original.PROBE,encoding='utf8')
    env=os.environ.copy();env.update(CARGO_TARGET_DIR=str(scratch/'target'),CARGO_INCREMENTAL='0')
    subprocess.run([env.get('CARGO','cargo'),'build','--manifest-path',str(scratch/'Cargo.toml'),'--offline','--release','--quiet'],check=True,env=env)
    return scratch/'target/release'/('systematic-distribution-probe.exe' if os.name=='nt' else 'systematic-distribution-probe')

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--probe',type=Path)
    parser.add_argument('--scratch',type=Path)
    parser.add_argument('--report',type=Path,required=True)
    parser.add_argument('--max-cases',type=int)
    args=parser.parse_args()
    sdk=Path(__file__).resolve().parents[2]
    before={str(p.relative_to(sdk)):hashlib.sha256(p.read_bytes()).hexdigest() for p in (sdk/'crates/math/src').glob('*.rs')}
    executable=args.probe.resolve() if args.probe else build_probe(sdk,args.scratch.resolve())
    rows=cases()[:args.max_cases]
    inputs=[];refs=[];warnings_seen=[]
    for label,d,rv,extra,family in rows:
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter('always')
            qs=np.asarray(rv.ppf(PS),dtype=float)
        qualified=[qualified_quantile(d,p,float(q)) for p,q in zip(PS,qs)]
        qs=np.asarray([q for q,_ in qualified],dtype=float)
        reference_kinds=[kind for _,kind in qualified]
        if caught:warnings_seen.append({'case':label,'categories':sorted({type(w.message).__name__ for w in caught})})
        xs=list(dict.fromkeys([float(x) for x in qs if math.isfinite(x)]+[float(x) for x in extra if math.isfinite(x)]))
        discrete=hasattr(rv,'pmf')
        if discrete:
            # Explicit steps immediately below/at/above the declared atoms.
            xs=list(dict.fromkeys(xs+[x+delta for x in xs[:5] for delta in [-1.,1.]]))
        with warnings.catch_warnings():
            warnings.simplefilter('ignore')
            cdfs=np.asarray(rv.cdf(xs),dtype=float)
            logs=np.asarray(rv.logpmf(xs) if discrete else rv.logpdf(xs),dtype=float)
            qcdf=np.asarray(rv.cdf(qs),dtype=float)
            qsf=np.asarray(rv.sf(qs),dtype=float)
            previous_cdf=np.asarray(rv.cdf(qs-1),dtype=float) if discrete else qcdf
            previous_sf=np.asarray(rv.sf(qs-1),dtype=float) if discrete else qsf
        qualified_cdfs=[qualified_cdf(d,x,float(value)) for x,value in zip(xs,cdfs)]
        cdfs=np.asarray([value for value,_ in qualified_cdfs])
        inputs.append(dict(distribution=d,probabilities=PS,xs=xs))
        refs.append((label,d,family,qs,xs,cdfs,logs,qcdf,qsf,previous_cdf,previous_sf,discrete,bool(caught),reference_kinds))
    run=subprocess.run([str(executable)],input=''.join(json.dumps(x,allow_nan=False)+'\n' for x in inputs),capture_output=True,text=True,timeout=900,check=True)
    outputs=[json.loads(line) for line in run.stdout.splitlines()]
    if len(outputs)!=len(rows):raise RuntimeError('Probe result count mismatch')
    counts=dict(parameterizations=len(rows),quantile_passes=0,cdf_passes=0,logdensity_passes=0,explicit_refusals=0,oracle_unavailable=0,tail_accuracy_targets=0,closed_form_quantile_references=0)
    failures=[];refusals=[];accuracy=[];family_counts={}
    def reject(label,op,actual):
        counts['explicit_refusals']+=1
        refusals.append(dict(case=label,operation=op,error=actual))
    def fail(label,d,op,expected,actual,**details):
        failures.append(clean(dict(case=label,distribution=d,operation=op,expected=expected,actual=actual,**details)))
    for ref,out in zip(refs,outputs):
        label,d,family,qs,xs,cdfs,logs,qcdf,qsf,previous_cdf,previous_sf,discrete,warned,reference_kinds=ref
        family_counts[family]=family_counts.get(family,0)+1
        if 'prepare_error' in out or 'parse_error' in out:
            reject(label,'prepare',out);continue
        if out['input_probabilities']!=PS or out['input_xs']!=xs:raise RuntimeError('Float transport changed oracle input')
        for p,expected,actual,reference_cdf,reference_sf,prev_cdf,prev_sf,reference_kind in zip(PS,qs,out['quantiles'],qcdf,qsf,previous_cdf,previous_sf,reference_kinds):
            # A rounded CDF close to one cannot validate a tail quantile.
            # Reference self-consistency uses the small tail directly; this
            # excludes inaccurate SciPy PPF cases independently of SDK output.
            target=p if p<=.5 else 1-p
            reference_tail=reference_cdf if p<=.5 else reference_sf
            margin=max(64*math.ulp(target),1e-12*target)
            # At a discrete jump, rounded CDFs can make SciPy's PPF choose a
            # neighboring atom. Demand a resolved bracketing inequality in
            # the small tail; ambiguous boundary points are not a reference.
            bracket_ok=(reference_cdf>p+margin and prev_cdf<p-margin) if p<=.5 else (reference_sf<target-margin and prev_sf>target+margin)
            if not math.isfinite(expected) or (reference_kind=='scipy' and (warned or (discrete and not bracket_ok) or (not discrete and abs(reference_tail-target)>max(32*math.ulp(target),1e-7*target)))):
                counts['oracle_unavailable']+=1;continue
            if reference_kind!='scipy':counts['closed_form_quantile_references']+=1
            if 'error' in actual:reject(label,'quantile',actual);continue
            tolerance=max(8*math.ulp(float(expected)),2e-6*abs(expected),1e-10 if expected==0 else 2e-306)
            if 'value' not in actual or abs(actual['value']-expected)>tolerance:fail(label,d,'quantile',expected,actual,p=p,tolerance=tolerance,reference_kind=reference_kind)
            else:counts['quantile_passes']+=1
        for x,expected,actual in zip(xs,cdfs,out['cdfs']):
            if not math.isfinite(expected):counts['oracle_unavailable']+=1;continue
            if 'error' in actual:reject(label,'cdf',actual);continue
            tolerance=5e-7 if d['kind'] in ['normal','lognormal'] else 2e-8
            if 'value' not in actual or abs(actual['value']-expected)>tolerance:fail(label,d,'cdf',expected,actual,x=x,tolerance=tolerance)
            else:counts['cdf_passes']+=1
            if 'value' in actual and 0<expected<1:
                target=max(32*math.ulp(float(expected)),3e-5*min(expected,1-expected))
                if abs(actual['value']-expected)>target:
                    counts['tail_accuracy_targets']+=1
                    accuracy.append(clean(dict(case=label,distribution=d,x=x,expected=expected,actual=actual['value'],target=target)))
        for x,expected,actual in zip(xs,logs,out['logdensities']):
            if 'error' in actual:reject(label,'logdensity',actual);continue
            if not math.isfinite(expected):
                if math.isinf(expected) and actual.get('nonfinite')==('-inf' if expected<0 else 'inf'):counts['logdensity_passes']+=1
                else:counts['oracle_unavailable']+=1
                continue
            tolerance=2e-6+1e-10*abs(expected)
            if 'value' not in actual or abs(actual['value']-expected)>tolerance:fail(label,d,'logdensity',expected,actual,x=x,tolerance=tolerance)
            else:counts['logdensity_passes']+=1
    after={str(p.relative_to(sdk)):hashlib.sha256(p.read_bytes()).hexdigest() for p in (sdk/'crates/math/src').glob('*.rs')}
    report=dict(schema='solvercraft.systematic-distribution-oracle.v1',probabilities=PS,counts=counts,family_parameterizations=family_counts,
                scipy=scipy.__version__,numpy=np.__version__,probe_sha256=hashlib.sha256(executable.read_bytes()).hexdigest(),
                harness_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),source_stable=before==after,
                failure_count=len(failures),failures=failures,refusals=clean(refusals),accuracy_targets=accuracy,oracle_warnings=warnings_seen,
                reference_qualification='90-digit elementary inverse/CDF formulas; exact symmetric medians; direct small-tail reference checks; independent positive normal/skew-normal quadrature; unresolved reference quantiles excluded',
                references=['https://docs.scipy.org/doc/scipy/reference/stats.html','https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.truncnorm.html',
                            'https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.invgauss.html','https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.skewnorm.html'])
    args.report.parent.mkdir(parents=True,exist_ok=True)
    args.report.write_text(json.dumps(report,indent=2,allow_nan=False)+'\n',encoding='utf8')
    print(json.dumps(dict(counts=counts,failure_count=len(failures),source_stable=before==after,first_failures=failures[:12]),indent=2),flush=True)
    return int(bool(failures))

if __name__=='__main__':raise SystemExit(main())
