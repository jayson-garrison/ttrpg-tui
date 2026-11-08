# A broken user override is dropped, never the built-in

Users may replace a built-in table or generator by id. If that override creates a cycle, dangling reference or too-deep nesting, the spec says to exclude the offender, but excluding a built-in would make shipped generators disappear because of a user file. We drop the user-origin items involved and restore the built-in they replaced; only when no built-in is involved are all members excluded, as the spec states. A built-in set on its own is always valid (tested). See D-21.
