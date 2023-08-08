import {
  IonActionSheet,
  IonButton,
  IonButtons,
  IonContent,
  IonHeader,
  IonIcon,
  IonPage,
  IonTitle,
  IonToolbar,
} from "@ionic/react";
import { addOutline } from "ionicons/icons";
import ExploreContainer from "../components/ExploreContainer";
import "./Home.css";
import { useHistory } from "react-router-dom";
import { RootState } from "../store";
import { useSelector } from "react-redux";
import { useEffect, useState } from "react";

const Home: React.FC = () => {
  const { setupDetailsState } = useSelector((state: RootState) => state);
  const history = useHistory();

  const { shinkai_identity, profile, registration_name, permission_type } =
    setupDetailsState;
  const displayString = `${shinkai_identity}/${profile}/${registration_name} (Device)`;
  const [showActionSheet, setShowActionSheet] = useState(false);

  useEffect(() => {
    console.log("Redux State:", setupDetailsState);
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
        <ExploreContainer />
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
              console.log("Create Chat clicked");
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
    </IonPage>
  );
};

export default Home;
