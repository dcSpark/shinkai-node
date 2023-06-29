Closing the TCP connection in a more graceful manner would usually involve following the standard TCP connection termination process, which consists of a four-way handshake (FIN, ACK, FIN, ACK). However, this process is typically handled by lower-level libraries, and you may not need to implement it manually.

In your case, you could attempt to enhance the process by ensuring both sides are ready to close the connection before actually closing it. Here's how:

1. **Client sends termination request to the server**: The client sends a special "terminate" message to the server.

2. **Server acknowledges termination request**: Upon receiving the "terminate" message, the server could send a "terminate_ack" message back to the client to acknowledge that it's ready to close the connection, and then stop its reading task.

3. **Client acknowledges server's termination acknowledgement**: The client, after receiving the "terminate_ack" message, can then also stop its reading task, effectively ending its side of the connection.

4. **Both sides close the connection**: Now, both the client and the server can close their sockets, since they have both agreed to terminate the connection.
