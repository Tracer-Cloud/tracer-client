#!/bin/bash

python3 -c '
import time

for i in range(5):
    print(f"Step {i+1}")
    time.sleep(1)
'
