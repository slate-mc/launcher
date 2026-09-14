import { describe, expect, it } from "vitest";
import { formatMinecraftSessionLog } from "./sessionLog";

describe("formatMinecraftSessionLog", () => {
  it("formats complete Log4j XML events and waits for partial events", () => {
    const timestamp = "1789347885576";
    const date = new Date(Number(timestamp));
    const time = [date.getHours(), date.getMinutes(), date.getSeconds()]
      .map((part) => part.toString().padStart(2, "0"))
      .join(":");
    const complete = `
      <log4j:Event logger="net.neoforged.fml.startup.Entrypoint" timestamp="${timestamp}" level="INFO" thread="main">
        <log4j:Message><![CDATA[JVM Uptime at startup: 179ms]]></log4j:Message>
      </log4j:Event>`;
    const partial = `
      <log4j:Event logger="partial" timestamp="${timestamp}" level="WARN" thread="main">
        <log4j:Message><![CDATA[not complete yet]]></log4j:Message>`;

    expect(formatMinecraftSessionLog(complete + partial)).toBe(
      `[${time}] [main/INFO] [net.neoforged.fml.startup.Entrypoint]: JVM Uptime at startup: 179ms`,
    );
  });

  it("decodes escaped messages and preserves throwable text", () => {
    const input = `<log4j:Event logger="example" timestamp="bad" level="error" thread="Render thread">
      <log4j:Message>Could not load &lt;entry&gt; &amp; fallback</log4j:Message>
      <log4j:Throwable><![CDATA[java.lang.IllegalStateException: broken
  at example.Main.run(Main.java:10)]]></log4j:Throwable>
    </log4j:Event>`;

    expect(formatMinecraftSessionLog(input)).toBe(
      "[--:--:--] [Render thread/ERROR] [example]: Could not load <entry> & fallback\njava.lang.IllegalStateException: broken\n  at example.Main.run(Main.java:10)",
    );
  });

  it("preserves ordinary process output", () => {
    expect(formatMinecraftSessionLog("JVM warning\nplain output\n")).toBe(
      "JVM warning\nplain output\n",
    );
  });

  it("drops an orphaned XML fragment from a bounded tail", () => {
    const input = `message tail]]></log4j:Message></log4j:Event>
      <log4j:Event logger="next" timestamp="0" level="INFO" thread="main">
        <log4j:Message><![CDATA[ready]]></log4j:Message>
      </log4j:Event>`;

    expect(formatMinecraftSessionLog(input)).toContain(
      "[main/INFO] [next]: ready",
    );
    expect(formatMinecraftSessionLog(input)).not.toContain("message tail");
  });
});
