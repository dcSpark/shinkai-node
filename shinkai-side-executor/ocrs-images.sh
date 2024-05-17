#!/bin/bash

image_files=(image-*-page-*.png)

for file in "${image_files[@]}"; do
  time ocrs $file -o ${file%.png}.txt
done