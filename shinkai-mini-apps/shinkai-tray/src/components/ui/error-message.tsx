const ErrorMessage = ({ message }: { message: string }) => {
  return (
    <div className="bg-red-500/10 text-red-700 text-sm px-4 py-2 rounded">
      <strong className="font-bold">Error: </strong>
      <span className="block sm:inline"> {message} </span>
    </div>
  );
};

export default ErrorMessage;
