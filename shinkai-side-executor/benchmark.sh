#!/bin/bash

mkdir -p results

# Export pages and OCR them
time cargo run -- --file ../files/Shinkai_Protocol_Whitepaper.pdf --extract

echo "OCR pages"
cd results
time ./../ocrs-pages.sh
cd ..

# Parse text and extract images
(time cargo run -- --file ../files/Shinkai_Protocol_Whitepaper.pdf > results/extracted_text.txt)  2>&1

echo "OCR images"
cd results
time ./../ocrs-images.sh
cd ..

# Generate layout for 1 page and all pages
time surya_layout  ../files/Shinkai_Protocol_Whitepaper.pdf --max 1 --results_dir results_1
time surya_layout  ../files/Shinkai_Protocol_Whitepaper.pdf

# OCR flowchart
time ocrs files/pdf-parsing-flowchart.png -o results/pdf-parsing-flowchart.txt