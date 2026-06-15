/* Frozen snapshot of a YouTube player.js blob (v123).
 * The decipher function shape is preserved verbatim so the regex
 * parser in src/provider/youtube/player_js.rs must continue to
 * match the canonical pattern. Any change to the regex MUST
 * update this fixture in lockstep.
 */
(function(a,b){a=a.split("");var c=a[0];a[0]=a[42%a.length];a[42%a.length]=c;c=a[1];a[1]=a[3];a[3]=c;c=a[2];a[2]=a[1];a[1]=c;a.reverse();return a.join("")})
