#!/usr/bin/env python3
import sys

output = open(sys.argv[1], "r").read()
answer = open(sys.argv[2], "r").read()

if output == answer:
    print("Accepted")
else:
    print("Wrong Answer")
