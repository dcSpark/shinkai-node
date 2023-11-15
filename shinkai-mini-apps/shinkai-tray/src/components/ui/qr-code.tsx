import React from "react";
import { QRCode as ReactQRCode } from "react-qrcode-logo";

import type { IProps } from "react-qrcode-logo";
import shinkaiLogo from "../../../app-icon.png";

export default function QRCode({
  value,
  size,
}: {
  value: IProps["value"];
  size: IProps["size"];
}): React.ReactElement {
  return (
    <ReactQRCode
      logoImage={shinkaiLogo}
      logoWidth={size ? size * 0.2 : undefined}
      value={value}
      eyeColor="black"
      eyeRadius={4}
      fgColor="black"
      size={size}
      removeQrCodeBehindLogo
      logoPaddingStyle="circle"
    />
  );
}
