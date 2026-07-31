#!/bin/bash
cd /home/brnx/Desktop/greplog || exit 1

if git status --porcelain | grep -q .; then
  git add -A
  git commit -m "auto-commit"
  git push origin main
fi
