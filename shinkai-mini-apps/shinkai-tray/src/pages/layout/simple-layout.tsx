const SimpleLayout = ({
  title,
  children,
}: {
  title: React.ReactNode;
  children: React.ReactNode;
}) => {
  return (
    <div className="py-10 max-w-lg mx-auto">
      <h1 className="text-center font-semibold tracking-tight text-2xl mb-8">{title}</h1>
      <div>{children}</div>
    </div>
  );
};

export default SimpleLayout;
