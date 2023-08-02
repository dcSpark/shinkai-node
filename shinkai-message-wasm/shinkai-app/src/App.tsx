import { Redirect, Route } from "react-router-dom";
import { IonApp, IonRouterOutlet, setupIonicReact } from "@ionic/react";
import { IonReactRouter } from "@ionic/react-router";
import Home from "./pages/Home";
import Chat from "./pages/Chat";
import ChatList from "./pages/ChatList";
import Settings from "./pages/Settings";

/* Core CSS required for Ionic components to work properly */
import "@ionic/react/css/core.css";

/* Basic CSS for apps built with Ionic */
import "@ionic/react/css/normalize.css";
import "@ionic/react/css/structure.css";
import "@ionic/react/css/typography.css";

/* Optional CSS utils that can be commented out */
import "@ionic/react/css/padding.css";
import "@ionic/react/css/float-elements.css";
import "@ionic/react/css/text-alignment.css";
import "@ionic/react/css/text-transformation.css";
import "@ionic/react/css/flex-utils.css";
import "@ionic/react/css/display.css";

/* Theme variables */
import "./theme/variables.css";
import Connect from "./pages/Connect";

setupIonicReact();

const App: React.FC = () => {
  const setupComplete = localStorage.getItem('setupComplete') === 'true';

  return (
    <IonApp>
      <IonReactRouter>
        <IonRouterOutlet>
          <Route path="/connect" component={Connect} />
          <Route exact path="/home" component={Home} />
          <Route path="/chatlist" component={ChatList} />
          <Route path="/chat/:id" component={Chat} />
          <Route path="/settings" component={Settings} />
          {!setupComplete && <Redirect exact from="/" to="/connect" />}
          <Redirect exact from="/" to="/home" />
        </IonRouterOutlet>
      </IonReactRouter>
    </IonApp>
  );
};

export default App;
