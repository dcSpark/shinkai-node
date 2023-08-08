import { IonPage, IonHeader, IonToolbar, IonTitle, IonContent, IonList, IonItem, IonLabel, IonButtons, IonBackButton } from '@ionic/react';
import React from 'react';

const AdminCommands: React.FC = () => {
  const commands = [
    'Get Peers',
    'Ping All',
    'Connect To',
    'Get Last Messages',
    'Create Registration Code',
    'Get All Subidentities'
  ];

  const handleCommandClick = (command: string) => {
    console.log(`Command selected: ${command}`);
    // You can handle each command individually here
    // e.g. if (command === 'Get Peers') { ... }
  };

  return (
    <IonPage>
      <IonHeader>
        <IonToolbar>
          <IonButtons slot="start">
            <IonBackButton defaultHref="/home" />
          </IonButtons>
          <IonTitle>Admin Commands</IonTitle>
        </IonToolbar>
      </IonHeader>
      <IonContent>
        <IonList>
          {commands.map(command => (
            <IonItem button key={command} onClick={() => handleCommandClick(command)}>
              <IonLabel>{command}</IonLabel>
            </IonItem>
          ))}
        </IonList>
      </IonContent>
    </IonPage>
  );
};

export default AdminCommands;
