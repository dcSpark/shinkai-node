import {
  IonBackButton,
  IonButton,
  IonButtons,
  IonContent,
  IonHeader,
  IonIcon,
  IonInput,
  IonItem,
  IonLabel,
  IonList,
  IonPage,
  IonText,
  IonTitle,
  IonToolbar,
} from "@ionic/react";
import { useParams } from "react-router-dom";
import { useEffect, useRef, useState } from "react";
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
import { ShinkaiMessage } from "../models/ShinkaiMessage";
import { calculateMessageHash } from "../utils/shinkai_message_handler";
import Avatar from "../components/ui/Avatar";
import { cn } from "../theme/lib/utils";
import { send } from "ionicons/icons";

const parseDate = (dateString: string) => {
  const formattedDateString =
    dateString.slice(0, 4) +
    "-" +
    dateString.slice(4, 6) +
    "-" +
    dateString.slice(6, 8) +
    "T" +
    dateString.slice(9, 11) +
    ":" +
    dateString.slice(11, 13) +
    ":" +
    dateString.slice(13, 15) +
    "Z";

  return new Date(Date.parse(formattedDateString));
};

const Chat: React.FC = () => {
  console.log("Loading Chat.tsx");
  useSetup();

  const dispatch = useDispatch();
  const setupDetailsState = useSelector(
    (state: RootState) => state.setupDetailsState,
  );

  const { id } = useParams<{ id: string }>();
  const bottomChatRef = useRef<HTMLDivElement>(null);
  const deserializedId = decodeURIComponent(id).replace(/~/g, ".");
  const [lastKey, setLastKey] = useState<string | undefined>(undefined);
  const [hasMoreMessages, setHasMoreMessages] = useState(true);
  const [prevMessagesLength, setPrevMessagesLength] = useState(0);

  const reduxMessages = useSelector(
    (state: RootState) => state.inboxes[deserializedId],
  );

  const [messages, setMessages] = useState<ShinkaiMessage[]>([]);
  const [inputMessage, setInputMessage] = useState("");
  const otherPersonIdentity = getOtherPersonIdentity(
    deserializedId,
    setupDetailsState.shinkai_identity,
  );

  useEffect(() => {
    dispatch(
      getLastMessagesFromInbox(deserializedId, 10, lastKey, setupDetailsState),
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

  useEffect(() => {
    // Check if the user is at the bottom of the chat
    const isUserAtBottom =
      bottomChatRef.current &&
      bottomChatRef.current.getBoundingClientRect().bottom <=
        window.innerHeight;

    // If the user is at the bottom, scroll to the bottom
    if (isUserAtBottom) {
      bottomChatRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [messages]);

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
        setupDetailsState,
      ),
    );
    setInputMessage("");
  };

  return (
    <IonPage>
      <IonHeader className="shadow">
        <IonToolbar>
          <IonButtons slot="start">
            <IonBackButton defaultHref="/home" />
          </IonButtons>
          <div className="flex gap-4 px-4">
            <IonTitle className="w-auto text-accent text-center">
              {otherPersonIdentity}
            </IonTitle>
            <Avatar />
          </div>
        </IonToolbar>
      </IonHeader>
      <IonContent fullscreen className="bg-neutral-50">
        <div className="container mx-auto">
          {hasMoreMessages && (
            <IonButton
              onClick={() =>
                dispatch(
                  getLastMessagesFromInbox(
                    deserializedId,
                    10,
                    lastKey,
                    setupDetailsState,
                    true,
                  ),
                )
              }
            >
              Load More
            </IonButton>
          )}
          <IonList className="flex flex-col gap-5 pt-6">
            {messages &&
              messages
                .slice()
                .reverse()
                .map((message, index) => (
                  <IonItem key={index} lines="none">
                    <div
                      className={cn(
                        "flex flex-col gap-1 max-w-[300px]",

                        true && "ml-auto",
                      )}
                    >
                      <IonLabel
                        className={
                          "rounded-xl bg-slate-50 shadow px-3 py-2 text-accent "
                        }
                      >
                        {message?.body?.content}
                      </IonLabel>
                      {message?.external_metadata?.scheduled_time && (
                        <IonText className="text-muted">
                          {parseDate(
                            message.external_metadata.scheduled_time,
                          ).toLocaleString()}
                        </IonText>
                      )}
                    </div>
                  </IonItem>
                ))}
          </IonList>
          <form
            className="flex gap-8 px-5 fixed bottom-0 left-0 right-0  pb-10 container"
            onSubmit={(e) => {
              e.preventDefault();
              if (inputMessage.trim() !== "") {
                sendMessage();
              }
            }}
          >
            <IonInput
              value={inputMessage}
              placeholder="Type a message..."
              shape="round"
              onIonChange={(e) => setInputMessage(e.detail.value!)}
            ></IonInput>
            <IonButton onClick={sendMessage} aria-label="Send Message">
              <IonIcon size="large" icon={send} />
            </IonButton>
          </form>
          <div ref={bottomChatRef} />
        </div>
      </IonContent>
    </IonPage>
  );
};

export default Chat;
