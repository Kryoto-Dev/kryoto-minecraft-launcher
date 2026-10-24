# Microsoft sign-in

The launcher signs players in with the OAuth **device-code flow**: it shows a short code, the
player enters it at microsoft.com/link in their own browser, and Microsoft gives the launcher a
token. The launcher never sees a password. The token is exchanged as Mojang documents it:
Xbox Live, then XSTS, then `api.minecraftservices.com/authentication/login_with_xbox`. The
account is added only when Minecraft's services confirm the game is owned and a profile exists.
Refresh tokens are kept in the system's credential store (Windows Credential Manager), never in
a file.

## What a build needs

An Azure app ID, passed at build time as `KRYOTO_MSA_CLIENT_ID` (a repository variable for the
release workflow). Without one, the build works and its Microsoft button says sign-in isn't set
up; offline profiles work either way.

1. In the Azure portal, App registrations, register an app for **personal Microsoft accounts
   only**. No redirect URI is needed.
2. Under Authentication, turn on **Allow public client flows** (the device-code flow).
3. Ask Mojang to allow the app to use Minecraft's services: Microsoft blocks
   `login_with_xbox` for app IDs that aren't approved (it answers 403 "Invalid app
   registration"). The form is linked from Mojang's help pages ("Minecraft Services API
   access"). Until it is approved, sign-in reaches Xbox Live and then stops with a message
   saying so.
4. Set the app ID as the `KRYOTO_MSA_CLIENT_ID` variable in the GitHub repository, and locally
   when building: `KRYOTO_MSA_CLIENT_ID=<id> pnpm app:build:local`.

The ID is not a secret (public clients have none), but it is Kryoto's: other projects must not
reuse it, and this launcher must not use another launcher's.
