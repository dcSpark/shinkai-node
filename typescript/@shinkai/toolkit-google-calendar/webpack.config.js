//webpack.config.js
const path = require('path');
// eslint-disable-next-line node/no-unpublished-require
const TsconfigPathsPlugin = require('tsconfig-paths-webpack-plugin');

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
    plugins: [new TsconfigPathsPlugin({})],
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
