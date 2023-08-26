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
import { IonContentCustom, IonHeaderCustom } from "../components/ui/Layout";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";

const CreateJob: React.FC = () => {
  const [jobName, setJobName] = useState("");

  const handleCreateJob = () => {
    // Perform your API request here
    console.log("Creating job with name:", jobName);
  };

  return (
    <IonPage>
      <IonHeaderCustom>
        <IonButtons slot="start">
          <IonBackButton defaultHref="/home" />
        </IonButtons>
        <IonTitle>Create Job</IonTitle>
      </IonHeaderCustom>
      <IonContentCustom>
        <IonGrid
          className={
            "rounded-[1.25rem] bg-white dark:bg-slate-800 p-4 md:p-10 space-y-2 md:space-y-4"
          }
        >
          <IonRow>
            <IonCol>
              <h2 className={"text-lg mb-3 md:mb-8 text-center"}>
                New Job Details
              </h2>
              <Input
                value={jobName}
                label="Enter Job Name"
                aria-label="Enter Job Name"
                onChange={(e) => setJobName(e.detail.value!)}
              />

              <div style={{ marginTop: "20px" }}>
                <Button onClick={handleCreateJob}>Create Job</Button>
              </div>
            </IonCol>
          </IonRow>
        </IonGrid>
      </IonContentCustom>
    </IonPage>
  );
};

export default CreateJob;
