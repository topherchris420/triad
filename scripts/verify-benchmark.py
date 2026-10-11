"""Independent NumPy calculation of Rust benchmark metrics and temporal intervals.
Run: python scripts/verify-benchmark.py REPORT [--plots DIRECTORY]
No Rust implementation is imported; this is computational, not scientific replication.
"""
import argparse
import hashlib
import json
from pathlib import Path
import numpy as np


def close(a, b):
    if b is None:
        assert a is None, (a, b)
    else:
        np.testing.assert_allclose(a, b, rtol=1e-11, atol=1e-12)


def metrics(rows, tau, actual, bins):
    score = np.array([r['uncertainty'] for r in rows])
    error = np.array([r['prediction'] != r['truth'] for r in rows], dtype=float)
    p = np.array([r['error_probability'] for r in rows])
    mask = score <= tau if tau is not None else np.zeros(len(rows), dtype=bool)
    check_counts(rows, error, mask, actual['operational'])
    order = np.argsort(score, kind='stable')
    ends = np.r_[np.flatnonzero(np.diff(score[order]) != 0)+1, len(rows)]
    risks = np.cumsum(error[order])[ends-1]/ends
    close(actual['aurc'], np.sum(np.diff(np.r_[0, ends])/len(rows)*risks))
    assert len(actual['risk_coverage_curve']) == len(ends)
    for row, n, risk in zip(actual['risk_coverage_curve'], ends, risks):
        close(row['coverage'], n/len(rows)); close(row['risk'], risk)
    close(actual['brier'], np.mean((p-error)**2))
    assignment = np.minimum((p*bins).astype(int), bins-1)
    ece = sum(abs(np.sum(p[assignment==i]-error[assignment==i])) for i in range(bins))/len(rows)
    close(actual['ece'], ece)
    for i, bin_row in enumerate(actual['reliability']):
        sel = assignment == i
        assert bin_row['count'] == sel.sum() and bin_row['events'] == error[sel].sum()
        close(bin_row['mean_probability'], float(p[sel].mean()) if sel.any() else None)
        close(bin_row['event_rate'], float(error[sel].mean()) if sel.any() else None)
    return error, mask


def check_counts(rows, error, mask, actual):
    assert actual['count'] == len(rows) and actual['accepted'] == mask.sum()
    assert actual['errors'] == error.sum() and actual['accepted_errors'] == error[mask].sum()
    close(actual['coverage'], mask.mean())
    close(actual['abstention_frequency'], 1-mask.mean())
    close(actual['selective_error_risk'], float(error[mask].mean()) if mask.any() else None)
    close(actual['incorrect_acceptance_frequency'], error[mask].sum()/len(rows))
    close(actual['error_rate'], error.mean())


def draws(seed, n):
    mask = (1 << 64)-1
    bound = ((1 << 64)-n) % n
    while True:
        seed = (seed + 0x9e3779b97f4a7c15) & mask
        z = seed
        z = ((z ^ (z >> 30)) * 0xbf58476d1ce4e5b9) & mask
        z = ((z ^ (z >> 27)) * 0x94d049bb133111eb) & mask
        z ^= z >> 31
        if z >= bound:
            yield z % n


def interval(days, block, reps, seed):
    if len(days)//block < 2:
        return None
    rng = draws(seed, len(days))
    values = []
    for _ in range(reps):
        selected = []
        while len(selected) < len(days):
            start = next(rng)
            selected.extend((start+np.arange(min(block,len(days)-len(selected)))) % len(days))
        n, a, ea, b, eb, _ = days[selected].sum(axis=0)
        ra = ea/a if a else np.nan
        rb = eb/b if b else np.nan
        values.append([ra, rb, rb-ra, a/n, b/n, ea/n, eb/n])
    values = np.array(values)
    totals = days.sum(axis=0)
    rare_a = totals[2] == 0 or totals[2] == totals[1]
    rare_b = totals[4] == 0 or totals[4] == totals[3]
    output = {}
    for i, name in enumerate(['risk_a','risk_b','risk_b_minus_a','coverage_a','coverage_b','incorrect_acceptance_a','incorrect_acceptance_b']):
        bad = (i in (0,2,5) and rare_a) or (i in (1,2,6) and rare_b) or not np.isfinite(values[:,i]).all()
        output[name] = None if bad else np.quantile(values[:,i], [.025,.975], method='linear')
    return output


def verify_intervals(days, actual, reps, seed):
    for result in actual:
        if result['ci95'] is None:
            assert result['status'] == 'insufficient_blocks'
            continue
        expected = interval(days, result['block_days'], reps, seed)
        for key, value in expected.items():
            close(result['ci95'][key], value)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('report', type=Path); p.add_argument('--plots', type=Path)
    args = p.parse_args()
    report = json.loads(args.report.read_bytes())['report']
    m = report['manifest']; size=m['rows_per_day']
    results = []; plot_data = []
    for model, actual in zip(m['models'],report['models']):
        inp = model['input']; generic = actual['conventional_and_selective']
        cal = [r for r in inp['records'] if r['split']=='calibration']
        rows = [r for r in inp['records'] if r['split']=='test']
        assert all(r['truth'] is not None for r in rows+cal), 'This independent real-data check requires complete labels'
        k = int(np.ceil(len(cal)*inp['target_coverage']))
        tau = sorted(r['uncertainty'] for r in cal)[k-1] if k else None
        close(generic['threshold'], tau)
        error, base = metrics(rows,tau,generic['overall'],inp['reliability_bins'])
        cal_error=np.array([r['prediction']!=r['truth'] for r in cal],float)
        cal_mask=np.array([tau is not None and r['uncertainty']<=tau for r in cal])
        check_counts(cal,cal_error,cal_mask,actual['calibration'])
        cal_risk=cal_error[cal_mask].mean() if cal_mask.any() else None
        wins=[w for w in model['windows'] if w['split']=='test']
        cwin=[w for w in model['windows'] if w['split']=='calibration']
        scores=lambda w: [max(w['covariate'],w['confidence']),w['covariate'],w['confidence']]
        thresholds=np.quantile(np.array([scores(w) for w in cwin]),m['alert_quantile'],axis=0,method='linear')
        flags=np.array([scores(w) for w in wins])>thresholds
        degrad=[]
        for w, wr in zip(wins,actual['windows']):
            sel=np.array([w['start_day']<=int(r['id'])//size<w['end_day'] for r in rows])
            subset=[r for r,s in zip(rows,sel) if s]
            metrics(subset,tau,generic['scenarios'][w['id']],inp['reliability_bins'])
            check_counts(subset,error[sel],base[sel],wr['selective'])
            risk=error[sel][base[sel]].mean() if base[sel].any() else None
            degraded=bool(risk>cal_risk+m['degradation_margin']) if risk is not None and cal_risk is not None else None
            assert wr['degraded']==degraded
            degrad.append(degraded)
        for j, policy in enumerate(actual['policies']):
            close(policy['threshold'],thresholds[j])
            assert policy['calibration_alerts']==sum(scores(w)[j]>thresholds[j] for w in cwin)
            assert policy['test_alerts']==flags[:,j].sum()
            gate=base & np.repeat(~flags[:,j],[(w['end_day']-w['start_day'])*size for w in wins])
            check_counts(rows,error,gate,policy['gated'])
            cf=[sum(d==True and f for d,f in zip(degrad,flags[:,j])),sum(d==False and f for d,f in zip(degrad,flags[:,j])),sum(d==True and not f for d,f in zip(degrad,flags[:,j])),sum(d==False and not f for d,f in zip(degrad,flags[:,j])),sum(d is None for d in degrad)]
            assert list(policy['confusion'].get(k) for k in ('true_positive','false_positive','false_negative','true_negative','unknown'))==cf
            days=np.column_stack([np.ones(len(rows)),base,error*base,gate,error*gate,error]).reshape(-1,size,6).sum(axis=1)
            verify_intervals(days,policy['paired_uncertainty'],inp['bootstrap_replicates'],inp['bootstrap_seed'])
        results.append((rows,error,base,inp))
        plot_data.append((actual,generic))
    for j, comparison in enumerate(report['paired_models'],1):
        rows,e,a,inp=results[0]; _,f,b,_=results[j]
        days=np.column_stack([np.ones(len(rows)),a,e*a,b,f*b,e]).reshape(-1,size,6).sum(axis=1)
        verify_intervals(days,comparison['intervals'],inp['bootstrap_replicates'],inp['bootstrap_seed'])
    # Every older artifact protected before this milestone must remain byte-identical.
    for name, sha in json.loads(Path('benchmarks/electricity/frozen-before-v6.json').read_bytes()).items():
        assert hashlib.sha256(Path(name).read_bytes()).hexdigest()==sha, name
    print('PASS: independent metrics, curves, bins, policies, ablations, paired temporal intervals and 73 frozen artifacts (tolerance 1e-11 relative / 1e-12 absolute).')
    if args.plots:
        import matplotlib
        matplotlib.use('Agg')
        import matplotlib.pyplot as plt
        args.plots.mkdir(parents=True,exist_ok=True)
        fig, axes=plt.subplots(2,2,figsize=(11,8),layout='constrained')
        for actual,generic in plot_data:
            name=actual['model_id']; overall=generic['overall']
            axes[0,0].plot([r['coverage'] for r in overall['risk_coverage_curve']],[r['risk'] for r in overall['risk_coverage_curve']],label=name)
            rel=[r for r in overall['reliability'] if r['count']]
            axes[0,1].plot([r['mean_probability'] for r in rel],[r['event_rate'] for r in rel],'o-',label=name)
            windows=actual['windows']; xx=[w['start_day'] for w in windows]
            axes[1,0].plot(xx,[w['selective']['selective_error_risk'] for w in windows],'o-',ms=3,label=name)
            axes[1,1].plot(xx,[w['selective']['coverage'] for w in windows],'o-',ms=3,label=name)
        axes[0,1].plot([0,.5],[0,.5],'k--',lw=1,label='identity')
        for ax,title,xlabel,ylabel in zip(axes.flat,['Risk–coverage (retrospective)','Error-probability reliability','Fixed-threshold selective risk','Fixed-threshold coverage'],['Coverage','Mean predicted error probability','Published stream day index','Published stream day index'],['Accepted error fraction','Observed error fraction','Accepted error fraction','Accepted fraction']):
            ax.set(title=title,xlabel=xlabel,ylabel=ylabel); ax.legend(); ax.grid(alpha=.2)
        fig.suptitle('OpenML Electricity 151 • frozen models • exploratory ordered-stream evaluation\nCalendar chronology unresolved; points are nonoverlapping windows, not independent trials',fontsize=12)
        fig.savefig(args.plots/'electricity.png',dpi=220); fig.savefig(args.plots/'electricity.pdf'); plt.close(fig)


if __name__=='__main__': main()
