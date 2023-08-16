import {
  IonActionSheet,
  IonAlert,
  IonButton,
  IonButtons,
  IonContent,
  IonHeader,
  IonIcon,
  IonItem,
  IonList,
  IonPage,
  IonTitle,
  IonToolbar,
} from "@ionic/react";
import { addOutline } from "ionicons/icons";
import ExploreContainer from "../components/ExploreContainer";
import "./Home.css";
import { useHistory } from "react-router-dom";
import { RootState } from "../store";
import { useDispatch, useSelector } from "react-redux";
import { useEffect, useState } from "react";
import { ApiConfig } from "../api/api_config";
import { clearStore } from "../store/actions";

const Home: React.FC = () => {
  const { setupDetailsState } = useSelector((state: RootState) => state);
  const history = useHistory();
  const dispatch = useDispatch();

  const { shinkai_identity, profile, registration_name, permission_type } =
    setupDetailsState;
  const displayString = `${shinkai_identity}/${profile}/${registration_name} (Device)`;
  const [showActionSheet, setShowActionSheet] = useState(false);
  const [showLogoutAlert, setShowLogoutAlert] = useState(false);
  const inboxes = useSelector((state: RootState) => state.inboxes);
  console.log("Inboxes:", inboxes);

  useEffect(() => {
    console.log("Redux State:", setupDetailsState);
    ApiConfig.getInstance().setEndpoint(setupDetailsState.node_address);
  }, []);

  return (
    <IonPage>
      <IonHeader>
        <IonToolbar>
          <IonTitle>{displayString}</IonTitle>
          <IonButtons slot="end">
            {" "}
            {/* Add the "+" button to the right side of the toolbar */}
            <IonButton onClick={() => setShowActionSheet(true)}>
              <IonIcon slot="icon-only" icon={addOutline} />
            </IonButton>
          </IonButtons>
        </IonToolbar>
      </IonHeader>
      <IonContent fullscreen>
        <IonHeader collapse="condense">
          <IonToolbar>
            <IonTitle size="large">{displayString}</IonTitle>
          </IonToolbar>
        </IonHeader>
        {/* <ExploreContainer /> */}
        <IonContent fullscreen>
          <IonHeader collapse="condense">
            <IonToolbar>
              <IonTitle size="large">{displayString}</IonTitle>
            </IonToolbar>
          </IonHeader>
          <IonList>
            {Object.entries(inboxes).map(([inboxId, inbox]) => (
              <IonItem
                key={inboxId}
                button
                onClick={() => history.push(`/chat/${inboxId}`)}
              >
                {inbox}
              </IonItem>
            ))}
          </IonList>
        </IonContent>
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
              history.push('/connect');
            },
          },
        ]}
      />
    </IonPage>
  );
};

export default Home;
