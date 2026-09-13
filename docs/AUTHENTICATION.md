# Minecraft authentication

slate is a native public OAuth client. It uses the system browser and the authorization-code
flow with PKCE; the desktop executable never contains or sends a Microsoft client secret.

## Application registration

The allow-listed Minecraft application registration must use these public values:

- Application client ID: `73823938-7288-4e14-9ede-33d98af655cf`
- Tenant: `consumers`
- Platform: Mobile and desktop applications
- Redirect URI: `http://localhost:38643`
- Public client flows: enabled

The redirect URI is intentionally fixed. Although Entra normally ignores ports for native
localhost redirects, the consumer-account handoff at `login.live.com` requires this application
registration to match the complete URI.

## Native flow

1. The Rust authentication crate binds `127.0.0.1:38643`, creates a one-time state value and PKCE
   verifier, and opens Microsoft authorization with `XboxLive.signin offline_access`.
2. The callback accepts only a loopback peer, validates the callback shape and state, and exchanges
   the single-use code with its PKCE verifier.
3. Rust exchanges the Microsoft token for Xbox User and XSTS tokens, requests a Minecraft Services
   token, verifies Java Edition entitlement, and loads the Minecraft profile. If Minecraft
   Services omits the selected skin, slate performs a best-effort lookup by the verified profile ID
   through Mojang's official public session server. Only the matching profile's pinned
   `textures.minecraft.net` skin URL is accepted and upgraded to HTTPS.
4. Only the Microsoft refresh token is retained, under an opaque reference in the operating-system
   credential vault. SQLite stores the profile ID, display name, status, and credential reference;
   it never stores OAuth, Xbox, or Minecraft tokens.
5. Every launch refreshes the chain and supplies the short-lived Minecraft session directly to the
   native launch planner. Renderer DTOs and logs never receive token values.

Only one sign-in callback can be active. A renderer reload reconnects to and reopens the existing
flow. If another process owns port 38643, slate reports the conflict and does not terminate that
process.
