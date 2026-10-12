# Implementation notes (fixed before the first real case was generated)

`protocol.json` leaves a few details open. The generator
(`scripts/m8-generate.py`) settles them as follows. Each choice was made before
any Milestone 8 model was fitted on real data and before any comparator existed.
Pipeline code paths were exercised only on synthetic data.

1. **Family D condition.** "Calendar month" means month of year
   (`month-of-year 11`). A year-month label could never match a past development
   period, which would make every claim degenerate.
2. **Family D weeks.** A week is used only if both the previous and the coming week
   lie entirely inside the evaluation period, and the coming week has at least 24
   eligible units. Otherwise the truth is undefined or rests on a handful of hours.
   `support_max` is the largest mean predicted error probability over Monday-start
   weeks that lie entirely inside the development period. Expected recent units are
   168 × number of series (12 for air quality).
3. **Design effect (family D).** ρ is the lag-1 autocorrelation of the
   development-period error indicator in unit order, computed per series (site for
   air quality) and averaged, then clipped to [0, 0.9].
4. **Moving-block bootstrap with one-cluster blocks** (families A-block and B):
   whole clusters, which here are days or record blocks, are resampled with
   replacement. With one-cluster blocks this coincides with the cluster bootstrap;
   the method label records how the evidence was collected (`block_bootstrap` for
   contiguous periods).
5. **Shift statistic for the gas data.** Predictions are class labels, so the PSI
   is computed over the six predicted classes rather than over deciles. Bank
   marketing uses the predicted subscription probability; the regression datasets
   use `y_hat`.
6. **Air-quality order.** Units are ordered by hour, then by site. Outcome records
   follow that order.
7. **Traffic holiday flag.** Set for every hour of a date that carries any holiday
   label other than `None`. The archive labels only the midnight row.
8. **Rounding.** Claim bounds of families A and B are rounded **up** to 6 decimals,
   so rounding never makes a claim stricter than its template. Family D bounds are
   `r_dev + d` rounded to 6 decimals.
9. **Dependence of conformal calibration (family C).** The random-split
   calibration half is a simple random sample of the month's units, so it is
   `independent` under the encoding rules. Whole-month calibration sets are
   `temporal`.
10. **Bank-marketing labels.** Month runs map to years by position (runs 1–7 →
    2008, 8–17 → 2009, 18–26 → 2010). Strata are `YYYY-MM`. A contact day is labelled
    `YYYY-MM-dayNNN`.
11. **Resampling clusters for scoring** are those named in `protocol.json`:
    A, one sample draw; B, one evidence window; C, one calibration month; D, one
    week.
