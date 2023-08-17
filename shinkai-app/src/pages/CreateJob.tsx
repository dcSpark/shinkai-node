// pages/CreateJob.tsx
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

const CreateJob: React.FC = () => {
  const [jobName, setJobName] = useState("");

  const handleCreateJob = () => {
    // Perform your API request here
    console.log("Creating job with name:", jobName);
  };

  return (
    <IonPage>
      <IonHeader>
        <IonToolbar>
          <IonButtons slot="start">
            <IonBackButton defaultHref="/home" />
          </IonButtons>
          <IonTitle>Create Job</IonTitle>
        </IonToolbar>
      </IonHeader>
      <IonContent className="ion-padding">
        <IonGrid>
          <IonRow>
            <IonCol>
              <h2>New Job Details</h2>
              <IonItem>
                <IonInput
                  value={jobName}
                  label="Enter Job Name"
                  aria-label="Enter Job Name"
                  placeholder="Post as Satoshi"
                  onIonChange={(e) => setJobName(e.detail.value!)}
                />
              </IonItem>
              <div style={{ marginTop: "20px" }}>
                <IonButton expand="full" onClick={handleCreateJob}>
                  Create Job
                </IonButton>
              </div>
            </IonCol>
          </IonRow>
        </IonGrid>
      </IonContent>
    </IonPage>
  );
};

export default CreateJob;
