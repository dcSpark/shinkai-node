//webpack.config.js
const path = require('path');

module.exports = {
  mode: 'development',
  devtool: 'inline-source-map',
  entry: {
    main: './src/registry.ts',
  },
  output: {
    iife: true,
    path: path.resolve(__dirname, './dist'),
    filename: 'packaged-shinkai-toolkit.js',
    libraryTarget: 'commonjs-module',
  },
  resolve: {
    extensions: ['.ts', '.tsx', '.js'],
  },
  module: {
    rules: [
      {
        test: /\.tsx?$/,
        loader: 'ts-loader',
      },
    ],
  },
  target: 'node',
};
