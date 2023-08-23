import React from "react";
import {
  IonBackButton,
  IonButtons,
  IonContent,
  IonFooter,
  IonHeader,
  IonTitle,
  IonToolbar,
} from "@ionic/react";
import "./Layout.css";

export const IonHeaderCustom = ({
  children,
}: {
  children: React.ReactNode;
}) => {
  return (
    <IonHeader className="shadow border-b border-slate-50 dark:border-slate-600">
      <IonToolbar className="mx-auto container" class="ion-header-custom">
        {children}
      </IonToolbar>
    </IonHeader>
  );
};
export const IonContentCustom = ({
  children,
}: {
  children: React.ReactNode;
}) => {
  return (
    <IonContent fullscreen class="ion-content-custom">
      <div className="container mx-auto">{children}</div>
    </IonContent>
  );
};

export const IonFooterCustom = ({
  children,
}: {
  children: React.ReactNode;
}) => {
  return (
    <IonFooter className="shadow border-t border-slate-50 dark:border-slate-600">
      <IonToolbar class="ion-toolbar-custom">{children}</IonToolbar>
    </IonFooter>
  );
};
