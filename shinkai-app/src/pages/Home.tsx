import {
  IonActionSheet,
  IonAlert,
  IonAvatar,
  IonButton,
  IonButtons,
  IonContent,
  IonHeader,
  IonIcon,
  IonItem,
  IonList,
  IonPage,
  IonText,
  IonTitle,
  IonToolbar,
} from "@ionic/react";
import { addOutline } from "ionicons/icons";
import "./Home.css";
import { useHistory } from "react-router-dom";
import { RootState } from "../store";
import { useDispatch, useSelector } from "react-redux";
import { useEffect, useState } from "react";
import { ApiConfig } from "../api/api_config";
import { clearStore } from "../store/actions";
import { getAllInboxesForProfile } from "../api";
import Avatar from "../components/ui/Avatar";
import { IonContentCustom, IonHeaderCustom } from "../components/ui/Layout";

const Home: React.FC = () => {
  const { setupDetailsState } = useSelector((state: RootState) => state);
  const history = useHistory();
  const dispatch = useDispatch();

  const { shinkai_identity, profile, registration_name, permission_type } =
    setupDetailsState;
  const displayString = (
    <>
      {`${shinkai_identity}/${profile}/${registration_name}`}{" "}
      <span className="text-muted text-sm">(Device)</span>
    </>
  );
  const [showActionSheet, setShowActionSheet] = useState(false);
  const [showLogoutAlert, setShowLogoutAlert] = useState(false);
  const inboxes = useSelector((state: RootState) => state.inboxes);
  console.log("Inboxes:", inboxes);

  useEffect(() => {
    console.log("Redux State:", setupDetailsState);
    ApiConfig.getInstance().setEndpoint(setupDetailsState.node_address);
  }, []);

  useEffect(() => {
    console.log("Redux State:", setupDetailsState);
    ApiConfig.getInstance().setEndpoint(setupDetailsState.node_address);

    // Local Identity
    const { shinkai_identity, profile, registration_name } = setupDetailsState;
    let sender = shinkai_identity;
    let sender_subidentity = `${profile}/device/${registration_name}`;

    // Assuming receiver and target_shinkai_name_profile are the same as sender
    let receiver = sender;
    let target_shinkai_name_profile = sender;

    dispatch(
      getAllInboxesForProfile(
        sender,
        sender_subidentity,
        receiver,
        target_shinkai_name_profile,
        setupDetailsState,
      ),
    );
  }, []);

  return (
    <IonPage>
      <IonHeaderCustom>
        <IonTitle className="text-center text-inherit">
          {displayString}
        </IonTitle>
        <IonButtons slot="end">
          {" "}
          {/* Add the "+" button to the right side of the toolbar */}
          <IonButton onClick={() => setShowActionSheet(true)}>
            <IonIcon slot="icon-only" icon={addOutline} />
          </IonButton>
        </IonButtons>
      </IonHeaderCustom>

      <IonContent fullscreen>
        <IonHeader collapse="condense">
          <IonToolbar>
            <IonTitle size="large">{displayString}</IonTitle>
          </IonToolbar>
        </IonHeader>
        {/* <ExploreContainer /> */}
        <IonContentCustom>
          {Object.entries(inboxes).map(([position, inboxId]) => (
            <IonItem
              key={position}
              button
              className="ion-item-home"
              onClick={() => {
                const encodedInboxId = position.toString().replace(/\./g, "~");
                history.push(`/chat/${encodeURIComponent(encodedInboxId)}`);
              }}
            >
              <Avatar className="shrink-0" />
              <IonText className="ml-4">{JSON.stringify(position)}</IonText>
            </IonItem>
          ))}
        </IonContentCustom>
      </IonContent>
      {/* Action Sheet (popup) */}
      <IonActionSheet
        isOpen={showActionSheet}
        onDidDismiss={() => setShowActionSheet(false)}
        buttons={[
          {
            text: "Admin Commands",
            role: permission_type !== "admin" ? "destructive" : undefined,
            handler: () => {
              if (permission_type === "admin") {
                history.push("/admin-commands");
              } else {
                console.log("Not authorized for Admin Commands");
              }
            },
          },
          {
            text: "Create Job",
            handler: () => {
              history.push("/create-job");
            },
          },
          {
            text: "Create Chat",
            handler: () => {
              history.push("/create-chat");
            },
          },
          {
            text: "Logout",
            role: "destructive",
            handler: () => {
              setShowLogoutAlert(true);
            },
          },
          {
            text: "Cancel",
            role: "cancel",
            handler: () => {
              console.log("Cancel clicked");
            },
          },
        ]}
      ></IonActionSheet>
      <IonAlert
        isOpen={showLogoutAlert}
        onDidDismiss={() => setShowLogoutAlert(false)}
        header={"Confirm"}
        message={
          "Are you sure you want to logout? This will clear all your data."
        }
        buttons={[
          {
            text: "Cancel",
            role: "cancel",
            handler: () => {
              console.log("Cancel clicked");
            },
          },
          {
            text: "Yes",
            handler: () => {
              dispatch(clearStore());
              history.push("/connect");
            },
          },
        ]}
      />
    </IonPage>
  );
};

export default Home;
