import {
  IonBackButton,
  IonButton,
  IonButtons,
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
import {
  getLastMessagesFromInbox,
  sendTextMessage,
  sendTextMessageWithInbox,
} from "../api/index";
import { RootState } from "../store";
import { useSetup } from "../hooks/usetSetup";
import {
  extractReceiverShinkaiName,
  getOtherPersonIdentity,
} from "../utils/inbox_name_handler";
import { ShinkaiMessageWrapper } from "../lib/wasm/ShinkaiMessageWrapper";
import { ShinkaiMessage } from "../models/ShinkaiMessage";
import { calculateMessageHash } from "../utils/shinkai_message_handler";

const Chat: React.FC = () => {
  console.log("Loading Chat.tsx");
  useSetup();

  const dispatch = useDispatch();
  const setupDetailsState = useSelector(
    (state: RootState) => state.setupDetailsState
  );

  const { id } = useParams<{ id: string }>();
  const deserializedId = decodeURIComponent(id).replace(/~/g, ".");
  const [lastKey, setLastKey] = useState<string | undefined>(undefined);
  const [hasMoreMessages, setHasMoreMessages] = useState(true);
  const [prevMessagesLength, setPrevMessagesLength] = useState(0);

  const reduxMessages = useSelector(
    (state: RootState) => state.inboxes[deserializedId]
  );

  const [messages, setMessages] = useState<ShinkaiMessage[]>([]);
  const [inputMessage, setInputMessage] = useState("");
  const otherPersonIdentity = getOtherPersonIdentity(
    deserializedId,
    setupDetailsState.shinkai_identity
  );

  useEffect(() => {
    dispatch(
      getLastMessagesFromInbox(deserializedId, 10, lastKey, setupDetailsState)
    );
    
  }, [id, dispatch, setupDetailsState]);

  useEffect(() => {
    if (reduxMessages && reduxMessages.length > 0) {
      const lastMessage = reduxMessages[reduxMessages.length - 1];
      const timeKey = lastMessage.external_metadata.scheduled_time;
      const hashKey = calculateMessageHash(lastMessage);
      const lastMessageKey = `${timeKey}:${hashKey}`;
      setLastKey(lastMessageKey);
      setMessages(reduxMessages);

      if (reduxMessages.length - prevMessagesLength < 10) {
        setHasMoreMessages(false);
      }
      setPrevMessagesLength(reduxMessages.length);
    }
  }, [reduxMessages]);

  const sendMessage = () => {
    console.log("Sending message: ", inputMessage);

    // Local Identity
    const { shinkai_identity, profile, registration_name } = setupDetailsState;
    let sender = shinkai_identity;
    let sender_subidentity = `${profile}/device/${registration_name}`;

    const receiver = extractReceiverShinkaiName(deserializedId, sender);
    console.log("Receiver:", receiver);

    dispatch(
      sendTextMessageWithInbox(
        sender,
        sender_subidentity,
        receiver,
        inputMessage,
        deserializedId,
        setupDetailsState
      )
    );
    setInputMessage("");
  };

  return (
    <IonPage>
      <IonHeader>
        <IonToolbar>
          <IonButtons slot="start">
            <IonBackButton defaultHref="/home" />
          </IonButtons>
          <IonTitle>Chat: {otherPersonIdentity}</IonTitle>
        </IonToolbar>
      </IonHeader>
      <IonContent fullscreen>
        {hasMoreMessages && (
          <IonButton
            onClick={() =>
              dispatch(
                getLastMessagesFromInbox(
                  deserializedId,
                  10,
                  lastKey,
                  setupDetailsState
                )
              )
            }
          >
            Load More
          </IonButton>
        )}
        <IonList>
          {messages &&
            messages
              .slice()
              .reverse()
              .map((message, index) => (
                <IonItem key={index}>
                  <IonLabel><pre>{JSON.stringify(message, null, 2)}</pre></IonLabel>
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
