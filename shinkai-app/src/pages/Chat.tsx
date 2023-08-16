import {
  IonButton,
  IonContent,
  IonHeader,
  IonInput,
  IonItem,
  IonLabel,
  IonList,
  IonPage,
  IonTitle,
  IonToolbar,
} from "@ionic/react";
import { useParams } from "react-router-dom";
import { useEffect, useState } from "react";
import { useDispatch, useSelector } from "react-redux";
import { getLastMessagesFromInbox, sendTextMessage } from "../api/index";
import { RootState } from "../store";
import { useSetup } from "../hooks/usetSetup";

const Chat: React.FC = () => {
  console.log("Loading Chat.tsx");
  useSetup();

  const dispatch = useDispatch();
  const setupDetailsState = useSelector(
    (state: RootState) => state.setupDetailsState
  );
  const { id } = useParams<{ id: string }>();
  const [messages, setMessages] = useState([]);
  const [inputMessage, setInputMessage] = useState("");

  useEffect(() => {
    const deserializedId = decodeURIComponent(id).replace(/~/g, '.');
    dispatch(getLastMessagesFromInbox(deserializedId, 10, undefined, setupDetailsState));
  }, [id, dispatch, setupDetailsState]);

  const sendMessage = () => {
    // Split shinkaiIdentity into sender and the rest
    let [receiver, ...rest] = id.split("/");

    // Join the rest back together to form sender_subidentity
    let receiver_subidentity = rest.join("/");

    // Local Identity
    const { shinkai_identity, profile, registration_name } = setupDetailsState;

    let sender = shinkai_identity;
    let sender_subidentity = `${profile}/device/${registration_name}`;

    dispatch(
      sendTextMessage(
        sender,
        sender_subidentity,
        receiver,
        receiver_subidentity,
        inputMessage,
        setupDetailsState
      )
    );
    setInputMessage("");
  };

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
        <IonList>
          {messages.map((message, index) => (
            <IonItem key={index}>
              <IonLabel>{JSON.stringify(message)}</IonLabel>
            </IonItem>
          ))}
        </IonList>
        <IonInput
          value={inputMessage}
          onIonChange={(e) => setInputMessage(e.detail.value!)}
        ></IonInput>
        <IonButton onClick={sendMessage}>Send</IonButton>
      </IonContent>
    </IonPage>
  );
};

export default Chat;
