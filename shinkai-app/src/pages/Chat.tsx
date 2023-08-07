import { IonContent, IonHeader, IonPage, IonTitle, IonToolbar } from '@ionic/react';
import { useParams } from 'react-router-dom';
import { useEffect } from 'react';

const Chat: React.FC = () => {
  const { id } = useParams<{ id: string }>();

  // We'll use an effect to handle the id change.
  useEffect(() => {
    // Here you would typically load or update your chat data using the id.
    console.log(`Chat id is ${id}`);
    // Note: Make sure to handle cleanup if necessary, for example canceling 
    // any outstanding network requests if the component is unmounted.
  }, [id]); // The effect is dependent on id, so it runs whenever id changes.

  return (
    <IonPage>
      <IonHeader>
        <IonToolbar>
          <IonTitle>Chat</IonTitle>
        </IonToolbar>
      </IonHeader>
      <IonContent fullscreen>
        {/* Your chat UI goes here */}
      </IonContent>
    </IonPage>
  );
};

export default Chat;
