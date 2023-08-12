import React, { useEffect, useState } from "react";
import { useDispatch, useSelector } from "react-redux";
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
import { BrowserQRCodeReader } from "@zxing/browser";
import { Camera, CameraResultType, CameraSource } from "@capacitor/camera";
import { useHistory } from "react-router-dom";
import { toast } from "react-toastify";
import type { AppDispatch, RootState } from "../store";
import { QrScanner } from "@yudiel/react-qr-scanner";
import { BarcodeScanner } from "@capacitor-community/barcode-scanner";
import { isPlatform } from "@ionic/react";
import {
  generateEncryptionKeys,
  generateSignatureKeys,
} from "../utils/wasm_helpers";
import { QRSetupData } from "../models/QRSetupData";
import { SetupDetailsState } from "../store/reducers";

export type MergedSetupType = SetupDetailsState & QRSetupData;

const Connect: React.FC = () => {
  const [setupData, setSetupData] = useState<MergedSetupType>({
    registration_code: "",
    profile: "main",
    registration_name: "main_device",
    identity_type: "device",
    permission_type: "admin",
    node_address: "",
    shinkai_identity: "",
    node_encryption_pk: "",
    node_signature_pk: "",
    myEncryptionPk: "",
    myEncryptionSk: "",
    myIdentityPk: "",
    myIdentitySk: "",
  });
  const [error, setError] = useState<string | null>(null);
  const dispatch = useDispatch<AppDispatch>();
  const history = useHistory();
  const errorFromState = useSelector((state: RootState) => state.error);

  // Generate keys when the component mounts
  useEffect(() => {
    // Assuming the seed is a random 32 bytes array.
    const seed = crypto.getRandomValues(new Uint8Array(32));
    generateEncryptionKeys(seed).then(
      ({ my_encryption_sk_string, my_encryption_pk_string }) =>
        setSetupData((prevState) => ({
          ...prevState,
          myEncryptionPk: my_encryption_pk_string,
          myEncryptionSk: my_encryption_sk_string,
        }))
    );
    generateSignatureKeys().then(
      ({ my_identity_pk_string, my_identity_sk_string }) =>
        setSetupData((prevState) => ({
          ...prevState,
          myIdentityPk: my_identity_pk_string,
          myIdentitySk: my_identity_sk_string,
        }))
    );
  }, []);

  const updateSetupData = (data: Partial<MergedSetupType>) => {
    setSetupData((prevState) => ({ ...prevState, ...data }));
  };

  const handleScan = async (data: any) => {
    if (data) {
      const result = JSON.parse(data);
      console.log("Prev. QR Code Data:", setupData);
      updateSetupData(result);
      console.log("New QR Code Data:", setupData);
    }
  };

  const handleImageUpload = async () => {
    try {
      const image = await Camera.getPhoto({
        quality: 90,
        allowEditing: true,
        resultType: CameraResultType.DataUrl,
        source: isPlatform("desktop")
          ? CameraSource.Photos
          : CameraSource.Prompt,
      });
      const codeReader = new BrowserQRCodeReader();
      const resultImage = await codeReader.decodeFromImageUrl(image.dataUrl);
      const json_string = resultImage.getText();
      const parsedData: QRSetupData = JSON.parse(json_string);
      updateSetupData(parsedData);
    } catch (error) {
      console.error("Error uploading image:", error);
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
    const success = await dispatch(submitRegistrationCode(setupData));

    if (success) {
      localStorage.setItem("setupComplete", "true");
      history.push("/home");
    } else {
      console.log("Error from state:", errorFromState);
      toast.error(errorFromState);
    }
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
            value={setupData.registration_code}
            onIonChange={(e) =>
              updateSetupData({ registration_code: e.detail.value! })
            }
            label="Registration Code"
            aria-label="Registration Code"
          />
        </IonItem>
        <IonItem>
          <IonInput
            value={setupData.registration_name}
            onIonChange={(e) =>
              updateSetupData({ registration_name: e.detail.value! })
            }
            label="Registration Name (Your choice)"
            aria-label="Registration Name (Your choice)"
          />
        </IonItem>
        <IonItem>
          <IonInput
            value={setupData.node_address}
            onIonChange={(e) =>
              updateSetupData({ node_address: e.detail.value! })
            }
            label="Node Address (IP:PORT)"
            aria-label="Node Address (IP:PORT)"
          />
        </IonItem>
        <IonItem>
          <IonInput
            value={setupData.shinkai_identity}
            onIonChange={(e) =>
              updateSetupData({ shinkai_identity: e.detail.value! })
            }
            label="Shinkai Identity (@@IDENTITY.shinkai)"
            aria-label="Shinkai Identity (@@IDENTITY.shinkai)"
          />
        </IonItem>
        <IonItem>
          <IonInput
            value={setupData.node_encryption_pk}
            onIonChange={(e) =>
              updateSetupData({ node_encryption_pk: e.detail.value! })
            }
            label="Node Encryption Public Key"
            aria-label="Node Encryption Public Key"
          />
        </IonItem>
        <IonItem>
          <IonInput
            value={setupData.node_signature_pk}
            onIonChange={(e) =>
              updateSetupData({ node_signature_pk: e.detail.value! })
            }
            label="Node Signature Public Key"
            aria-label="Node Signature Public Key"
          />
        </IonItem>
        <IonItem>
          <IonInput
            value={setupData.myEncryptionPk}
            onIonChange={(e) =>
              updateSetupData({ myEncryptionPk: e.detail.value! })
            }
            label="My Encryption Public Key"
            aria-label="My Encryption Public Key"
          />
        </IonItem>
        <IonItem>
          <IonInput
            value={setupData.myIdentityPk}
            onIonChange={(e) =>
              updateSetupData({ myIdentityPk: e.detail.value! })
            }
            label="My Signature Public Key"
            aria-label="My Signature Public Key"
          />
        </IonItem>
        <IonButton onClick={handleImageUpload}>Upload Image</IonButton>
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
