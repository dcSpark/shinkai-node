import { Link } from "react-router-dom";

import { LucideArrowLeft } from "lucide-react";

import { HOME_PATH } from "../../routes/name";

const SimpleLayout = ({
  title,
  children,
}: {
  title: React.ReactNode;
  children: React.ReactNode;
}) => {
  return (
    <div className="py-10 max-w-lg mx-auto">
      <Link className="absolute left-10" to={HOME_PATH}>
        <LucideArrowLeft />
        <span className="sr-only">Back</span>
      </Link>
      <h1 className="text-center font-semibold tracking-tight text-2xl mb-8">{title}</h1>
      <div>{children}</div>
    </div>
  );
};

export default SimpleLayout;
