/* Frozen snapshot of a YouTube player.js blob (v456) — n-parameter
 * extraction target. The decipher function is unchanged from
 * v123 so we focus the fixture on the ncode helper. The helper
 * lives in the same player blob; the regex parser in
 * src/provider/youtube/player_js.rs must continue to extract the
 * n_code string from this shape.
 *
 * The actual n_code string the player emits for v456 was captured
 * verbatim and is the input to the unit tests in
 * src/provider/youtube/ncode.rs.
 */
var ncode = "2D03AC2F8B9A6E1D7F0C5B4E9A8D3F2C1B0A9E8D7C6B5A4F3E2D1C0B9A8F7E6D5C4B3A2F1E0D";
(function(a,b){a=a.split("");var c=a[0];a[0]=a[42%a.length];a[42%a.length]=c;c=a[1];a[1]=a[3];a[3]=c;c=a[2];a[2]=a[1];a[1]=c;a.reverse();return a.join("")})
