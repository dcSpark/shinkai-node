cd typescript/@shinkai/

cd toolkit-lib \
&& npm ci 
cd ..

cd toolkit-runner \
&& npm ci \
&& npm run compile \
&& cp dist/shinkai-toolkit-executor.js ../../../files
cd ..

cd toolkit-example \
&& npm ci \
&& npm run compile \
&& cp dist/packaged-shinkai-toolkit.js ../../../files
