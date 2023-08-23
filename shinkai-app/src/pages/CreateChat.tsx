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
  IonTextarea,
} from "@ionic/react";
import { useState } from "react";
import { useDispatch, useSelector } from "react-redux";
import { sendTextMessage } from "../api";
import { useSetup } from "../hooks/usetSetup";
import { RootState } from "../store/reducers";
import { useHistory } from "react-router-dom";
import { History } from "history";

const CreateChat: React.FC = () => {
  useSetup();
  const setupDetailsState = useSelector(
    (state: RootState) => state.setupDetailsState
  );
  const [shinkaiIdentity, setShinkaiIdentity] = useState("");
  const [messageText, setMessageText] = useState("");
  const dispatch = useDispatch();
  const history: History<unknown> = useHistory();

  const handleCreateChat = async () => {
    // Perform your API request here
    console.log("Creating chat with Shinkai Identity:", shinkaiIdentity);

    // Split shinkaiIdentity into sender and the rest
    let [receiver, ...rest] = shinkaiIdentity.split("/");

    // Join the rest back together to form sender_subidentity
    let receiver_subidentity = rest.join("/");

    // Local Identity
    const { shinkai_identity, profile, registration_name } =
      setupDetailsState;

    let sender = shinkai_identity;
    let sender_subidentity = `${profile}/device/${registration_name}`;
    // console.log("Sender:", shinkai_identity);
    // console.log("Sender Subidentity:", `${profile}/device/${registration_name}`);

    // Send a message to someone
    let inboxId = await dispatch(
      sendTextMessage(
        sender,
        sender_subidentity,
        receiver,
        receiver_subidentity,
        messageText,
        setupDetailsState
      )
    );

    if (inboxId) {
      // Hacky solution because react-router can't handle dots in the URL
      const encodedInboxId = inboxId.toString().replace(/\./g, '~');
      history.push(`/chat/${encodeURIComponent(encodedInboxId)}`);
    }
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
              <h2>New Chat Details</h2>
              <IonItem>
                <IonInput
                  value={shinkaiIdentity}
                  label="Enter Shinkai Identity"
                  aria-label="Enter Shinkai Identity"
                  placeholder="@@name.shinkai or @@name.shinkai/profile"
                  onIonChange={(e) => setShinkaiIdentity(e.detail.value!)}
                />
              </IonItem>
              <IonItem>
                <IonLabel position="floating">Enter Message</IonLabel>
                <IonTextarea
                  label="Enter Message"
                  aria-label="Enter Message"
                  value={messageText}
                  onIonChange={(e) => setMessageText(e.detail.value!)}
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
