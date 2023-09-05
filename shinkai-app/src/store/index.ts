import { createStore, applyMiddleware, Store } from 'redux';
import thunk, { ThunkAction } from 'redux-thunk';
import storage from 'redux-persist/lib/storage';
import { persistStore, persistReducer } from 'redux-persist';
import rootReducer from './reducers';
import { Action } from 'redux';

export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = Store['dispatch'];

export type AppThunk<ReturnType = void> = ThunkAction<
  ReturnType,
  RootState,
  unknown,
  Action<string>
>;

const persistConfig = {
  key: 'root',
  storage,
  whitelist: ['other', 'setupDetails', "messages"],
  debug: true,
};

const persistedReducer = persistReducer(persistConfig, rootReducer);

export const store = createStore(persistedReducer, applyMiddleware(thunk));
export const persistor = persistStore(store);
