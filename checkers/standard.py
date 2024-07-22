#!/usr/bin/env python3
import sys

output = open(sys.argv[1], "r").read().split('\n')
answer = open(sys.argv[2], "r").read().split('\n')

def trim(s):
    for i in range(len(s)):
        s[i] = s[i].rstrip()
    while len(s) > 0 and s[-1] == "":
        s.pop()

trim(output)
trim(answer)

if "\n".join(output) == "\n".join(answer):
    print("Accepted")
else:
    print("Wrong Answer")
