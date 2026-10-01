#!/usr/bin/env python3
"""票 06：把 results/memory/<label>.tsv（t_s fp_MB rows done [cpu_s]）汇总成一行。
load 期 ≈ t 3..28 s（20 s 静置后才开始采样，snap idle 约 1-2 s，oha 25 s）。
用法: mem06_curve.py <label>...
"""
import sys, os
D = os.path.join(os.path.dirname(__file__), "../results/memory")
print("label\tpeak_MB\tt_peak\tt_rows_final\tfp@30\tfp@45\tfp@60\tfp@90\tfp@last")
for lb in sys.argv[1:]:
    rows = [l.split("\t") for l in open(f"{D}/{lb}.tsv") if l.strip()]
    rows = [(int(r[0]), float(r[1]), int(r[2]) if r[2] != "-" else 0) for r in rows]
    at = lambda t: min(rows, key=lambda r: abs(r[0] - t))[1]
    pk = max(rows, key=lambda r: r[1]); final = rows[-1][2]
    tf = next(r[0] for r in rows if r[2] == final)
    print(f"{lb}\t{pk[1]:.0f}\t{pk[0]}\t{tf}\t{at(30):.0f}\t{at(45):.0f}\t{at(60):.0f}\t{at(90):.0f}\t{rows[-1][1]:.0f}")
