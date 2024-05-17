#!/bin/bash

page_files=(exported-page-*.png)

for file in "${page_files[@]}"; do
  time ocrs $file -o ${file%.png}.txt
done