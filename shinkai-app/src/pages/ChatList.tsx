import { IonContent, IonHeader, IonPage, IonTitle, IonToolbar } from '@ionic/react';

const ChatList: React.FC = () => {
  return (
    <IonPage>
      <IonHeader>
        <IonToolbar>
          <IonTitle>Chats</IonTitle>
        </IonToolbar>
      </IonHeader>
      <IonContent fullscreen>
        {/* Add your chat list components here */}
      </IonContent>
    </IonPage>
  );
};

export default ChatList;
