# Partner Manager
A Discord bot to help manage partnerships with other Discord servers.

## Functionality
The bot primarily does two things:

- Embed management: The bot will maintain a partnership embed and update it automatically with changes in partnerships.
- Role management: The bot will also manage who has your partnership role and keep that list synchronized with the list of partner representatives you've given it.

## Required Server Permissions
- Send Messages
- Manage Roles

The partner management bot role must also be ranked higher than the partner role for your server.

## Required Intents
In order to synchronize the partner role, the Guild Members intent must be configured.