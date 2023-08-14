## Group Chats

The implementation of group chats is not as simple as 1on1 chats. The main complexity is related to keeping the group chat history (particularly admin related actions) in sync between all the participants. The following sections describe the implementation of group chats in detail.

### Group Chat Requirements

- Multiple people should be able to send messages to each other (even encrypted).
- The group chat should be able to be created by any user.
- The group chat should allow users to be added and removed by admins.
- Admins can add other admins.
- Admins can remove other admins.

### Group Chat Creation

- Only one user is the actual creator. This user generates the group chat inbox_name which is used to identify the group chat.
- InboxName: group_chat::chat_uuid::creation_time::hash_of_initial_sorted_identities::creator_shinkai_id
- The creator is the first admin of the group chat.
- Besides the normal inbox_name used for the chat an action_group_inbox is created where all the admin actions are stored.

### Group Admin Actions

- Add user
- Remove user
- Update Group Description
- Add Admin (user already needs to have been added to the group)
- Remove Admin

### Messages

- Messages are normal ShinkaiMessages for most of it.
- The exception is that the sender needs to always send the id / time of the action when it was added to the action_group_inbox.
- Similar for admin actions, the sender needs to send the id / time of the action when it was added to the action_group_inbox.

WIP