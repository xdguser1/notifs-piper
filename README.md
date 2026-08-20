# Description
Notifs-piper is yet another notification daemon. It aims to replace [statnot](https://github.com/halhen/statnot), as it is extremely limiting in its capabilities. Even though [the protocol](https://specifications.freedesktop.org/notification/latest/protocol.html) is aimed at graphical notification daemons such as [dunst](https://github.com/dunst-project/dunst), this throws that idea out of the window and lets users decide how they actually want to receive their notifications.

This is done with a simple idea: separating the notification daemon into a server-client relationship. Most, if not all, notification daemons integrate the GUI components directly with what connects to dbus. This is limiting if you want to integrate notifications in your own apps as it usually requires object extensions to connect to dbus and listen for notifications. On the other hand, `statnot` will simply forward notifications, without actually implementing the hard parts of the protocol, such as signaling. Thus, there is a need for a notification daemon that lets apps listen on dbus, without having their own dbus extensions, whist still managing signaling, events and perhaps logging.

# Usage
## Server
To start the notification daemon, simply execute `notifs-piper daemon`. By default, the server will not send `NotificationClosed` events when a notification has timed out. To add this capability, add the `--auto-close` or `-c` option to `notifs-piper daemon`.

There can only be one server listening on the object `org.freedesktop.Notifications`, so if `busctl --user list` contains another service, `notifs-piper` will fail.

The server will log notifications for multiple sessions. There is, however, a maximum number of notifications that can be stored at all times. By default, 100 notifications will be stored inbetween sessions. You can change this with the `--max` or `-m` option when launching `notifs-piper daemon`.

Finally, you may want some more capabilities (such as allowing sound, actions, body, etc.) to the server. By default, none are activated. You can add them all with `--all` (or `-a`), but you may also want to only select a few with `--option opt` (or `-o opt`) where `opt` is the additional capability.

Other options can be found with the `--help` option on all commands and in [the official documention](https://specifications.freedesktop.org/notification/latest/protocol.html).

## Client
### Notifications
There are two commands for connecting, then receiving, notifications.

The first one is simply `notifs-piper read n` which will read `n` notifications from the logs (most recent to less) and return a JSON array (see the protocol subsection). You can add the option `--skip n` or `-s n` to skip the first `n` most recent notifications.

The second one will act as a tiny notification server. It will receive notifications sent to `org.freedesktop.Notifications`' `notify` function, as well as any other signals and errors. You can run this with the command `notifs-piper watch`.

By default, notifications will be marked as `read` once they are either read with `notifs-piper read` or `notifs-piper watch`. You may not want this, if, for example, you have a service that reads all unread notifications and sends them to a private server somewhere. To not mark a notification as read while running these commands, you can add the `--silent` or `-q` option.

### Signaling
This server supports signaling. For example, if you implement your own GUI on top of this, you may want to add a 'close button' which closes the notification (and thus hides it) everywhere. To do this, you may simply use the `notifs-piper signal n s` where `n` is the notification id and `s` is the given signal (either `closed`, `activation-token` or `action-invoked` subcommands).

Note that you may not use a notification id which has previously been closed, as this will result in **undefined behaviour** based solely on the protocol. To quote [the specs](https://specifications.freedesktop.org/notification/latest/protocol.html):

> The ID specified in the signal is invalidated before the signal is sent and may not be used in any further communications with the server.

To prevent this kind of mistake, by default, the server will check that:
1. The notification exists in its logs, and
2. The notification was not closed

If both are valid, the signal will emit as normal. If one, or both, of them is false, the signal will not be emitted and whatever command you used will silently fail.

In the rare case where the notification is valid (e.g. the client stores 11 notifications, but the daemon only 10), you may use the `--force` or `-f` option to force the signal to be emitted.

To check whether a notification was previously closed (or does not exist in the logs), you can use `notifs-piper signal n closed --query` or `notifs-piper signal n closed -q`, where `n` is the notification id, to check if that notification was closed previously. Note that it does **NOT** follow from this that the next time, even if invoked immediately, the result of this command will be the same if the result returned is `false`. This is because the notification may have been closed during the transmission of the results to the user. However, any `true` result will still be `true`, no matter how much time has passed.

### Protocol
Any result will be in the following format: `type#((result)|(subtype#result))`

The first part (`type`) can one of three options:
1. `res`: the result of the operation.
2. `ev`: an event from signaling.
3. `oth`: something else. Usually internal errors.
4. `err`: an error in the execution.

If the `type` is not `ev`, the second part will be the result of the operation (e.g. a JSON list for `notifs-piper read n`, or a notification in JSON format for `notifs-piper watch`). If the `type` is `ev`, then it signifies a signal was emitted. In that case, `subtype` can be one of the following:
1. `cls`: the notification was closed. The result is `id` which is the notification id.
2. `acv`: an activation token. The result is `id#activation` where `id` is the notification id and `activation` is a string representing the token.
3. `act`: an action invoked. The result is `id#action` where `id` is the notification id and `action` is a string representing the action.

Here are a few examples for a better understanding.
- `ev#cls#1` means `the notification with id 1 is now closed`
- `res#[{...}]` means `the result of the operation is the JSON list of the representation of a notification`
- `ev#act#2#Ok` means `the notification with id 2 has had an action represented by "Ok" invoked on it`
- `oth#Internal error: server is down` means `Something has a result of "Internal error: server is down"`

If you just want to show notifications without doing anything else, you can just use `notifs-piper watch | grep "^res" | cut -c5-` and that will print each new notification on a new line in JSON format.

# Debugging and architecture
Feel free to skip this section if you are not interested in the code.

The server is made out of three main parts:
1. The dbus implementation (or `NotificationsWrapper`)
2. The execution manager (or `LogsManager`)
3. The listening service (or `Listener`)

They communicate via two paths. The first one is through the `Listener`. The `Listener` is set to listen on a socket for any connections. When a client connects to the socket, the `Listener` will keep the connection open and create a new `Job` for the `LogsManager` by pushing the `Job` at the end of the second path of communication: the `SyncList` queue.

`SyncList`, as its name implies, is a synchronized list (technically, it's an `Arc` with a `Mutex` on a `VecDeque`). It is the main way to execute requests. For example, the `NotificationsWrapper` will send a `Broadcast` `Job` to the `SyncList` when `org.freedesktop.Notifications`' `notify` is called.

Now, to get back to the server, the first part simply is a [zbus](https://docs.rs/zbus/latest/zbus/) interface on `org.freedesktop.Notifications`. It acts as a wrapper for signals and pushes `Job`s to the `SyncList`.

The second part is the `LogsManager`. As its name implies, it manages the internal logs and is in charge of executing the `Job`s. Once these are finished, the results will be passed through the socket to the `Listener` which will broadcast to all listening processes the results if it is a signal or a `Broadcast` connection, or, if it's an individual `Job` such as `read`, it will transmit the results to the process directly.

So, tldr, we have the following:
```
Client request:                                  [  => NotificationsWrapper  ]
client => socket => Listener => SyncList => Manager => socket => Listener => socket => client

Call to notify:
NotificationsWrapper => SyncList => Manager => socket => Listener => socket => clients
```

For the cli, [clap](https://docs.rs/clap/latest/clap/) is used. For the asynchronous functions, [tokio](https://tokio.rs/) is used.

For anything relating debugging, you can simply add the `--debug` or `-d` option for `notifs-piper` and debugging will be enabled.

# Contributing
Any help is welcome. There is no special rules, but only broad concepts (except two which must be respected):
- Your code shouldn't be something you put in a deobfuscator. It doesn't _need_ to be 'clean code, 30 lines max, one function per function' or whatever, but it must be clear at what it does.
- Your code should be the best you can do. If you're not happy with it, neither will be the others that use it.
- Your code should be general enough, but doesn't need to have unnecessary abstractions.
- Etc. I'm sure you get the gist of it.

Now for the two rules:
- Use `cargo fmt --all` before pushing. I don't have a coding convention; I delegate it to the rust devs.
- Understand your code. Don't just say: `Claude, please refactor everything`. That's not because it won't be good at it, it's because maintainers need to understand the code they have written in case of a problem. They also need to understand why something works or doesn't. Every rule has an exception, but when you don't know the rules, you can't understand the exceptions.

Now I'm sure you can understand these simple things. I won't die on a hill if those are not respected; you can see for yourself the questionable quality of some of my decisions and how it may break some 'rule'. So long as I don't lose myself in lines and lines of `if` statements, it's probably fine.

If you see mistakes in grammar, please open an issue about it, because it may not be an error (canadian english is weird, so 'neighbour' is fine by me, but so is 'neighbor').

Anyways, many thanks for those who contribute, and many thanks to those that use this.
