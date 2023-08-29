const path = require('path');
// eslint-disable-next-line node/no-unpublished-require
const TerserPlugin = require('terser-webpack-plugin');

module.exports = {
  mode: 'production', // Change mode to 'production',
  stats: 'errors-warnings',
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
  optimization: {
    minimize: true,
    minimizer: [
      new TerserPlugin({
        terserOptions: {
          compress: false, // Turn off compression
          mangle: false, // Turn off mangling
          output: {
            comments: false,
            beautify: false, // Removes unnecessary whitespace
          },
        },
      }),
    ],
  },

  target: 'node',
};
