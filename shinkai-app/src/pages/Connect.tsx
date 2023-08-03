import React, { useEffect, useState } from "react";
import { useDispatch } from "react-redux";
import {
  IonContent,
  IonHeader,
  IonPage,
  IonTitle,
  IonToolbar,
  IonButton,
  IonInput,
  IonLabel,
  IonItem,
  IonToast,
} from "@ionic/react";
import { submitRegistrationCode } from "../api";
import { useHistory } from "react-router-dom";
import type { AppDispatch } from "../store";
import { QrScanner } from "@yudiel/react-qr-scanner";
import { BarcodeScanner } from "@capacitor-community/barcode-scanner";
import { isPlatform } from "@ionic/react";
import {
  generateEncryptionKeys,
  generateSignatureKeys,
} from "../utils/wasm_helpers";

const Connect: React.FC = () => {
  const [code, setCode] = useState("");
  const [profileName, setProfileName] = useState("");
  const [identityPk, setIdentityPk] = useState("");
  const [encryptionPk, setEncryptionPk] = useState("");
  const [error, setError] = useState<string | null>(null);
  const dispatch = useDispatch<AppDispatch>();
  const history = useHistory();

  // Generate keys when the component mounts
  useEffect(() => {
    // Assuming the seed is a random 32 bytes array.
    const seed = crypto.getRandomValues(new Uint8Array(32));
    generateEncryptionKeys(seed).then(({ my_encryption_pk_string }) =>
      setEncryptionPk(my_encryption_pk_string)
    );
    generateSignatureKeys().then(({ my_identity_pk_string }) =>
      setIdentityPk(my_identity_pk_string)
    );
  }, []);

  const handleScan = async (data: any) => {
    if (data) {
      const result = JSON.parse(data);
      setCode(result.code);
      setProfileName(result.profileName);
      setIdentityPk(result.identityPk);
      setEncryptionPk(result.encryptionPk);
    }
  };

  const handleError = (err: any) => {
    console.error(err);
  };

  const handleQRScan = async () => {
    if (isPlatform("capacitor")) {
      const result = await BarcodeScanner.startScan();
      if (result.hasContent) {
        handleScan(result.content);
      }
    }
  };

  const finishSetup = async () => {
    // TODO: finish this
  //   await dispatch(
  //     submitRegistrationCode(code, profileName, identityPk, encryptionPk)
  //   );
    localStorage.setItem("setupComplete", "true");
    history.push("/home");
  };

  return (
    <IonPage>
      <IonHeader>
        <IonToolbar>
          <IonTitle>Connect</IonTitle>
        </IonToolbar>
      </IonHeader>
      <IonContent fullscreen>
        {error && <IonToast color="danger" message={error} duration={2000} />}
        <IonItem>
          <IonInput
            value={code}
            onIonChange={(e) => setCode(e.detail.value!)}
            label="Code"
            aria-label="Code"
          />
        </IonItem>
        <IonItem>
          <IonInput
            value={profileName}
            onIonChange={(e) => setProfileName(e.detail.value!)}
            label="Profile Name"
            aria-label="Profile Name"
          />
        </IonItem>
        <IonItem>
          <IonInput
            value={identityPk}
            onIonChange={(e) => setIdentityPk(e.detail.value!)}
            label="Identity Public Key"
            aria-label="Identity Public Key"
          />
        </IonItem>
        <IonItem>
          <IonInput
            value={encryptionPk}
            onIonChange={(e) => setEncryptionPk(e.detail.value!)}
            label="Encryption Public Key"
            aria-label="Encryption Public Key"
          />
        </IonItem>
        {isPlatform("capacitor") ? (
          <IonButton onClick={handleQRScan}>Scan QR Code</IonButton>
        ) : (
          <QrScanner
            scanDelay={300}
            onError={handleError}
            onDecode={handleScan}
            containerStyle={{ width: "100%" }}
          />
        )}
        <IonButton onClick={finishSetup}>Finish Setup</IonButton>
      </IonContent>
    </IonPage>
  );
};

export default Connect;
