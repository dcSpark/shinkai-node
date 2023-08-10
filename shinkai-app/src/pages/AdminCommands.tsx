import {
  IonModal,
  IonButton,
  IonPage,
  IonHeader,
  IonToolbar,
  IonTitle,
  IonContent,
  IonList,
  IonItem,
  IonLabel,
  IonButtons,
  IonBackButton,
  IonActionSheet,
  IonToast,
} from "@ionic/react";
import React, { useState } from "react";
import { useDispatch, useSelector } from "react-redux";
import { submitCreateRegistrationCode } from "../api";
import { RootState } from "../store";
import { clearRegistrationCode } from "../store/actions";
import { useSetup } from "../hooks/usetSetup";

const AdminCommands: React.FC = () => {
  useSetup();
  const [showCodeRegistrationActionSheet, setShowCodeRegistrationActionSheet] =
    useState(false);
  const [showCodeRegistrationModal, setCodeRegistrationShowModal] =
    useState(false);
  const dispatch = useDispatch();
  const registrationCode = useSelector(
    (state: RootState) => state.registrationCode
  );
  const commands = [
    "Get Peers",
    "Ping All",
    "Connect To",
    "Get Last Messages",
    "Create Registration Code",
    "Get All Subidentities",
  ];

  const handleCommandClick = (command: string) => {
    console.log(`Command selected: ${command}`);

    if (command === "Create Registration Code") {
      setShowCodeRegistrationActionSheet(true);
    }
  };

  const copyToClipboard = () => {
    navigator.clipboard.writeText(registrationCode);
  };

  const handleIdentityClick = async (identityType: string) => {
    await dispatch(submitCreateRegistrationCode(identityType));
    setCodeRegistrationShowModal(true); // Show the modal after the registration code is created
    return true;
  };

  return (
    <>
      <IonActionSheet
        isOpen={showCodeRegistrationActionSheet}
        onDidDismiss={() => setShowCodeRegistrationActionSheet(false)}
        buttons={[
          {
            text: "Admin",
            handler: () => handleIdentityClick("admin"),
          },
          {
            text: "Standard",
            handler: () => handleIdentityClick("standard"),
          },
          {
            text: "None",
            handler: () => handleIdentityClick("none"),
          },
          {
            text: "Cancel",
            role: "cancel",
          },
        ]}
      />
      <IonModal isOpen={showCodeRegistrationModal}>
        <IonHeader>
          <IonToolbar>
            <IonTitle>Code Registration Successful</IonTitle>
          </IonToolbar>
        </IonHeader>
        <IonContent className="ion-padding">
          <IonLabel>Code: {registrationCode}</IonLabel>
          <div
            style={{
              display: "flex",
              justifyContent: "center",
              marginTop: "20px",
            }}
          >
            <IonButton
              onClick={copyToClipboard}
              style={{ marginRight: "10px" }}
            >
              Copy
            </IonButton>
            <IonButton onClick={() => setCodeRegistrationShowModal(false)}>
              Dismiss
            </IonButton>
          </div>
        </IonContent>
      </IonModal>
      <IonPage>
        <IonHeader>
          <IonToolbar>
            <IonButtons slot="start">
              <IonBackButton defaultHref="/home" />
            </IonButtons>
            <IonTitle>Admin Commands</IonTitle>
          </IonToolbar>
        </IonHeader>
        <IonContent>
          <IonList>
            {commands.map((command) => (
              <IonItem
                button
                key={command}
                onClick={() => handleCommandClick(command)}
              >
                <IonLabel>{command}</IonLabel>
              </IonItem>
            ))}
          </IonList>
        </IonContent>
      </IonPage>
    </>
  );
};

export default AdminCommands;
