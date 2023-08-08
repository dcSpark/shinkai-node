// // Connect.test.tsx

// import React from 'react';
// import { render, fireEvent, screen } from '@vitest/utils-dom';
// import { Provider } from 'react-redux';
// import { PersistGate } from 'redux-persist/integration/react';
// import { store, persistor } from '../store'; // Adjust the path accordingly
// import Connect from './Connect';  // Adjust the path to your Connect component

// describe('Connect Component with Persistence', () => {
//     it('persists setupData across sessions', async () => {
//         // First render - Initial session
//         render(
//             <Provider store={store}>
//                 <PersistGate loading={null} persistor={persistor}>
//                     <Connect />
//                 </PersistGate>
//             </Provider>
//         );

//         // Simulate some actions that modify setupData.
//         // Adjust according to your real UI interactions.
//         const button = screen.getByText('Scan QR Code');
//         await fireEvent.click(button);

//         // Here we simulate the "restart" of the application by re-rendering the component.
//         // This is a simplistic way to mimic persistence, and it assumes that redux-persist immediately writes to storage.
//         render(
//             <Provider store={store}>
//                 <PersistGate loading={null} persistor={persistor}>
//                     <Connect />
//                 </PersistGate>
//             </Provider>
//         );

//         // Check if the changes persist.
//         // For this example, let's assume node_address is shown in the UI after scan.
//         const persistedData = screen.getByText('The expected persisted node_address value'); // Replace with your expected value
//         expect(persistedData).toBeInTheDocument();
//     });
// });
