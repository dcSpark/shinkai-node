git clone https://github.com/skeskinen/bert.cpp
cd bert.cpp
git submodule update --init --recursive
mkdir build
cd build
cmake .. -DBUILD_SHARED_LIBS=OFF -DCMAKE_BUILD_TYPE=Release
make
cd ..
cp build/bin/server ../bert-cpp-server
cd ..
rm -rf bert.cpp
