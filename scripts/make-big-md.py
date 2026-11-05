#!/usr/bin/env python3
"""Writes a large Markdown file for measuring the big-file stages.

    python scripts/make-big-md.py OUT.md LINES   # 10000 ~ 1 MiB, 60000 ~ 6.5 MiB, 400000 ~ 42 MiB
"""
import sys

out, n = sys.argv[1], int(sys.argv[2])
lines = []
for i in range(n):
    if i % 200 == 0:
        lines.append(f"\n# Section {i // 200}\n")
    elif i % 50 == 0:
        lines.append(f"\n## Part {i // 50}\n")
    elif i % 7 == 0:
        lines.append(f"- item {i} 日本語の項目 **bold** `code`")
    else:
        lines.append(f"Line {i}: the quick brown fox jumps over the lazy dog. 日本語の文章もここに入れて長さを稼ぎます。")
with open(out, "w", encoding="utf-8", newline="\n") as f:
    f.write("\n".join(lines) + "\n")
