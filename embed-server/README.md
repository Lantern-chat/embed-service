Embed Server
============

# Config
See [`config.toml`](config.toml) for the default template config.

If `signed = true` (default), a signing key is required somewhere in the environment or in a `.env` file. You must explicitly set it to `false` to disable.

## Environment Variables

#### `CAMO_SIGNING_KEY`

128-bit media-url signing key encoded as a hexadecimal string. For Lantern, this is used to verify media URLs when they are proxied through our camo-worker.

#### `EMBEDS_BIND_ADDRESS`

IP Address for the microservice to bind to.

# Usage

Use the HTTP `POST` method to send URLs to the service, with the URL in the body of the message.

You may also append a `?lang=en-US` (example) query parameter to the request URI to set the `Accept-Language` header with the given locale when fetching the embed.

### Example

```bash
EMBEDS_BIND_ADDRESS="127.0.0.1:8050" cargo run

curl --request POST \
  --url http://localhost:8050/ \
  --header 'Content-Type: text/plain' \
  --data 'https://www.youtube.com/watch?v=7v62m2KgwR8'
```
returns
<details>
    <summary>
    Click to expand JSON
    </summary>

```json
[
	"2023-05-17T19:24:45.455Z",
	{
		"v": "1",
		"ts": "2023-05-17T19:09:45.455Z",
		"ty": "vid",
		"u": "https://www.youtube.com/watch?v=7v62m2KgwR8",
		"t": "Emerald Dream | Hollow Knight Blind #4",
		"d": "Please refrain from posting spoilers or offering unsolicited gameplay advice in the comments. Thanks for watching my Let's Play of Hollow Knight!Hollow Knigh...",
		"ac": 16711680,
		"au": {
			"n": "About Oliver",
			"u": "https://www.youtube.com/@AboutOliver"
		},
		"p": {
			"n": "YouTube",
			"u": "https://www.youtube.com/",
			"i": {
				"u": "https://www.youtube.com/s/desktop/edbfd7e1/img/favicon_144x144.png",
				"h": 144,
				"w": 144
			}
		},
		"img": {
			"u": "https://i.ytimg.com/vi/7v62m2KgwR8/maxresdefault.jpg",
			"h": 720,
			"w": 1280
		},
		"vid": {
			"u": "https://www.youtube.com/embed/7v62m2KgwR8",
			"h": 720,
			"w": 1280,
			"m": "text/html"
		},
		"thumb": {
			"u": "https://i.ytimg.com/vi/7v62m2KgwR8/hqdefault.jpg",
			"h": 360,
			"w": 480
		}
	}
]
```
</details>

# License
Licensed under the terms of the [GNU Affero General Public License](https://www.gnu.org/licenses/agpl-3.0.en.html) as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version. See [LICENSE](LICENSE) for more details.