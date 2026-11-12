#!/bin/bash
# Times typical jobs of the command line on large generated survey data, with peak memory.
#
#   scripts/bench.sh [BINARY] [JOB...]
#
# BINARY defaults to target/release/statscraft (build it with cargo build --release first).
# The data is generated with Statscraft's own syntax into $BENCH_DIR (default: a new folder
# under /tmp), reused when it is there, and never belongs in the repository:
#   a.sav  1 000 000 cases x 51 variables (numbers, labelled categories, two strings)
#   b.sav     50 000 cases x 2 000 labelled variables
#   c.sav    200 000 cases with 30 dichotomies and 5 categories in two multiple response sets
# Every job runs under a 5 GB memory cap. Prints one line per job: wall seconds, CPU seconds,
# peak memory and exit code; the output of each job is kept in $BENCH_DIR/out/ to compare runs.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
bin=$(realpath "${1:-$root/target/release/statscraft}")
shift $(($# > 0 ? 1 : 0))
dir=${BENCH_DIR:-$(mktemp -d /tmp/statscraft-bench-XXXX)}
mkdir -p "$dir/jobs" "$dir/out"
cd "$dir"

cap=(systemd-run --user --scope -q -p MemoryMax=5G -p MemorySwapMax=0 --)

# one generated case per iteration of the input program, then the variables column by column
cases() {
    printf "SET SEED=%s.\nINPUT PROGRAM.\nLOOP #i = 1 TO %s.\nCOMPUTE id = #i.\nEND CASE.\nEND LOOP.\nEND FILE.\nEND INPUT PROGRAM.\n" "$1" "$2"
}
gen() {
    [ -f "$1.sav" ] && return
    echo "generating $1.sav"
    cat > "jobs/gen_$1.sps"
    /usr/bin/time -f "gen_$1 %e s %M KB" "$bin" run "jobs/gen_$1.sps" > /dev/null || exit 1
}
{
    cases 1 1000000
    cat <<'EOF'
NUMERIC x1 TO x30 (F8.2) /q1 TO q15 (F2.0) /region gender agegrp (F1.0).
STRING city (A12) /note (A20).
VECTOR x = x1 TO x30 /q = q1 TO q15.
LOOP #j = 1 TO 30.
COMPUTE x(#j) = RND(RV.NORMAL(50, 10) * 100) / 100.
END LOOP.
LOOP #j = 1 TO 15.
COMPUTE q(#j) = TRUNC(RV.UNIFORM(1, 6)).
IF RV.UNIFORM(0, 1) < 0.03 q(#j) = 9.
END LOOP.
COMPUTE region = TRUNC(RV.UNIFORM(1, 6)).
COMPUTE gender = TRUNC(RV.UNIFORM(1, 3)).
COMPUTE agegrp = TRUNC(RV.UNIFORM(1, 7)).
COMPUTE city = CONCAT('city', STRING(TRUNC(RV.UNIFORM(1, 40)), F2.0)).
COMPUTE note = CONCAT('n', STRING(TRUNC(RV.UNIFORM(1, 1000)), F4.0)).
MISSING VALUES q1 TO q15 (9).
VALUE LABELS q1 TO q15 1 'Very bad' 2 'Bad' 3 'Neutral' 4 'Good' 5 'Very good' 9 'No answer'
  /region 1 'North' 2 'South' 3 'East' 4 'West' 5 'Centre' /gender 1 'Male' 2 'Female'
  /agegrp 1 '15-24' 2 '25-34' 3 '35-44' 4 '45-54' 5 '55-64' 6 '65+'.
SAVE OUTFILE='a.sav'.
EOF
} | gen a
{
    cases 2 50000
    cat <<'EOF'
NUMERIC v1 TO v1999 (F2.0).
VECTOR v = v1 TO v1999.
LOOP #j = 1 TO 1999.
COMPUTE v(#j) = TRUNC(RV.UNIFORM(1, 8)).
END LOOP.
MISSING VALUES v1 TO v1999 (7).
VALUE LABELS v1 TO v1999 1 'Strongly disagree' 2 'Disagree' 3 'Somewhat disagree'
  4 'Neither agree nor disagree' 5 'Somewhat agree' 6 'Agree' 7 'Do not know'.
SAVE OUTFILE='b.sav'.
EOF
} | gen b
{
    cases 3 200000
    cat <<'EOF'
NUMERIC m1 TO m30 (F1.0) /c1 TO c5 (F2.0) /region gender agegrp (F1.0) /w (F8.4) /sat (F8.2).
VECTOR m = m1 TO m30 /c = c1 TO c5.
LOOP #j = 1 TO 30.
COMPUTE m(#j) = RV.UNIFORM(0, 1) < 0.1 + #j / 100.
END LOOP.
LOOP #j = 1 TO 5.
COMPUTE c(#j) = TRUNC(RV.UNIFORM(1, 21)).
IF RV.UNIFORM(0, 1) < 0.3 c(#j) = $SYSMIS.
END LOOP.
COMPUTE region = TRUNC(RV.UNIFORM(1, 6)).
COMPUTE gender = TRUNC(RV.UNIFORM(1, 3)).
COMPUTE agegrp = TRUNC(RV.UNIFORM(1, 7)).
COMPUTE w = RV.UNIFORM(0.5, 1.5).
COMPUTE sat = RV.NORMAL(7, 2).
VALUE LABELS m1 TO m30 0 'Not chosen' 1 'Chosen' /c1 TO c5 1 'Brand 1' 2 'Brand 2' 3 'Brand 3'
  4 'Brand 4' 5 'Brand 5' 6 'Brand 6' 7 'Brand 7' 8 'Brand 8' 9 'Brand 9' 10 'Brand 10'
  /region 1 'North' 2 'South' 3 'East' 4 'West' 5 'Centre' /gender 1 'Male' 2 'Female'
  /agegrp 1 '15-24' 2 '25-34' 3 '35-44' 4 '45-54' 5 '55-64' 6 '65+'.
MRSETS /MDGROUP NAME=$aware LABEL='Brands known' VARIABLES=m1 TO m30 VALUE=1
  /MCGROUP NAME=$bought LABEL='Brands bought' VARIABLES=c1 TO c5.
SAVE OUTFILE='c.sav'.
EOF
} | gen c

job() { printf '%s\n' "$2" > "jobs/$1.sps"; }
job a_get "GET FILE='a.sav'."
job a_save_sav "GET FILE='a.sav'.
SAVE OUTFILE='a2.sav'."
job a_save_zsav "GET FILE='a.sav'.
SAVE OUTFILE='a2.zsav' /ZCOMPRESSED."
job a_get_zsav "GET FILE='a2.zsav'."
job a_compute_recode "GET FILE='a.sav'.
DO REPEAT x = x1 TO x30 /y = y1 TO y30.
COMPUTE y = x * 2 + LN(ABS(x) + 1).
END REPEAT.
RECODE q1 TO q15 (1,2=1) (3=2) (4,5=3) (ELSE=SYSMIS) INTO r1 TO r15.
RECODE x1 TO x30 (LO THRU 40=1) (40 THRU 60=2) (60 THRU HI=3).
EXECUTE."
job a_aggregate "GET FILE='a.sav'.
AGGREGATE /OUTFILE=* /BREAK=region gender agegrp /m1 TO m10 = MEAN(x1 TO x10)
  /s1 TO s5 = SD(x11 TO x15) /n = N.
EXECUTE."
job a_sort "GET FILE='a.sav'.
SORT CASES BY region city (D) x1.
EXECUTE."
job a_match "GET FILE='a.sav'.
SAVE OUTFILE='left.sav' /KEEP=id x1 TO x15.
SAVE OUTFILE='right.sav' /KEEP=id x16 TO x30 q1 TO q15.
MATCH FILES /FILE='left.sav' /FILE='right.sav' /BY id.
EXECUTE."
job a_freq "GET FILE='a.sav'.
FREQUENCIES VARIABLES=q1 TO q15 region gender agegrp city."
job a_desc "GET FILE='a.sav'.
DESCRIPTIVES VARIABLES=x1 TO x30 q1 TO q15 /STATISTICS=MEAN STDDEV MIN MAX SKEWNESS KURTOSIS."
job a_crosstabs "GET FILE='a.sav'.
CROSSTABS /TABLES=q1 TO q5 BY region BY gender /CELLS=COUNT ROW COLUMN /STATISTICS=CHISQ."
job a_means "GET FILE='a.sav'.
MEANS TABLES=x1 TO x10 BY region BY gender /CELLS=MEAN COUNT STDDEV."
job a_regression "GET FILE='a.sav'.
REGRESSION /DEPENDENT=x1 /METHOD=ENTER x2 TO x20 q1 TO q5."
job a_factor "GET FILE='a.sav'.
FACTOR /VARIABLES=x1 TO x30 q1 TO q15 region gender agegrp id /EXTRACTION=PC /ROTATION=VARIMAX."
job b_get "GET FILE='b.sav'."
job b_save "GET FILE='b.sav'.
SAVE OUTFILE='b2.sav'."
job b_freq200 "GET FILE='b.sav'.
FREQUENCIES VARIABLES=v1 TO v200."
job b_desc "GET FILE='b.sav'.
DESCRIPTIVES VARIABLES=v1 TO v1999."
job b_recode_count "GET FILE='b.sav'.
RECODE v1 TO v1999 (1,2=1) (3,4,5=2) (6=3).
COMPUTE total = SUM(v1 TO v1999).
COUNT agree = v1 TO v1999 (5,6).
EXECUTE."
job b_factor "GET FILE='b.sav'.
FACTOR /VARIABLES=v1 TO v50 /EXTRACTION=PC."
job c_ctables "GET FILE='c.sav'.
CTABLES /TABLE \$aware [COUNT COLPCT.COUNT] BY gender [C] + agegrp [C]
  /SIGTEST TYPE=CHISQUARE /COMPARETEST TYPE=PROP ALPHA=0.05.
CTABLES /TABLE region [C] > gender [C] [COUNT ROWPCT.COUNT] + \$bought BY agegrp [C]
  /CATEGORIES VARIABLES=region gender agegrp TOTAL=YES /COMPARETEST TYPE=PROP.
CTABLES /TABLE sat [MEAN STDDEV] BY region [C] > gender [C] /COMPARETEST TYPE=MEAN.
CTABLES /TABLE \$aware BY \$bought."
job c_mult_response "GET FILE='c.sav'.
WEIGHT BY w.
MULT RESPONSE GROUPS=aware (m1 TO m30 (1)) /FREQUENCIES=aware.
CROSSTABS /TABLES=c1 BY region BY gender."

run() {
    local name=$1
    shift
    /usr/bin/time -f "%e %U %S %M" -o "out/$name.time" "${cap[@]}" "$bin" "$@" > "out/$name.md" 2> "out/$name.err"
    local code=$? secs user sys kb
    read -r secs user sys kb < <(tail -1 "out/$name.time")
    printf "%-18s %8.2f s %8.2f cpu %6d MB  exit %d\n" "$name" "$secs" \
        "$(echo "$user + $sys" | bc)" "$((kb / 1024))" "$code"
}
names=("$@")
[ ${#names[@]} -eq 0 ] && names=(a_get a_save_sav a_save_zsav a_get_zsav a_compute_recode
    a_aggregate a_sort a_match a_freq a_desc a_crosstabs a_means a_regression a_factor
    b_get b_save b_freq200 b_desc b_recode_count b_factor c_ctables c_mult_response session)
for name in "${names[@]}"; do
    if [ "$name" = session ]; then
        # a session that loads a.sav, then a run that loads it, changes it and saves it again
        rm -rf .statscraft
        run session_first run -s bench -e "GET FILE='a.sav'."
        run session_next run -s bench -e $'COMPUTE zz = 1.\nEXECUTE.'
        rm -rf .statscraft
    else
        run "$name" run "jobs/$name.sps"
    fi
done
echo "data and outputs: $dir"
