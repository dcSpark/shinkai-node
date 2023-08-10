// pages/CreateChat.tsx
import {
    IonPage,
    IonHeader,
    IonToolbar,
    IonTitle,
    IonContent,
    IonButton,
    IonInput,
    IonLabel,
    IonItem,
    IonGrid,
    IonRow,
    IonCol,
    IonButtons,
    IonBackButton,
  } from "@ionic/react";
  import { useState } from "react";
  
  const CreateChat: React.FC = () => {
    const [otherUser, setOtherUser] = useState("");
  
    const handleCreateChat = () => {
      // Perform your API request here
      console.log("Creating chat with user:", otherUser);
    };
  
    return (
      <IonPage>
        <IonHeader>
          <IonToolbar>
            <IonButtons slot="start">
              <IonBackButton defaultHref="/home" />
            </IonButtons>
            <IonTitle>Create Chat</IonTitle>
          </IonToolbar>
        </IonHeader>
        <IonContent className="ion-padding">
          <IonGrid>
            <IonRow>
              <IonCol>
                <h2>New Job Details</h2>
                <IonItem>
                  <IonInput
                    value={otherUser}
                    label="Enter Other User's"
                    aria-label="Enter Other User's"
                    placeholder="@@user.shinkai"
                    onIonChange={(e) => setOtherUser(e.detail.value!)}
                  />
                </IonItem>
                <div style={{ marginTop: "20px" }}>
                  <IonButton expand="full" onClick={handleCreateChat}>
                    Create Chat
                  </IonButton>
                </div>
              </IonCol>
            </IonRow>
          </IonGrid>
        </IonContent>
      </IonPage>
    );
  };
  
  export default CreateChat;
  