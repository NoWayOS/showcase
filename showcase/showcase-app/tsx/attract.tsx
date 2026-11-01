// The showcase's attract loop: after a while with no input on the
// launcher, slides take its place in turn, each one example's or idea's
// title and line, from the showcase's own text (slots). The first key,
// click or touch wakes the launcher and presses nothing. Every brand is
// fictional; no field, and nothing kept. Each slide's title is on a header
// bar in its example's brand colour.
import { Column, HeaderBar, Notice, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 16;
const EDGE = 48;
const TIGHT = 16;

type AttractProps = {
  title: string;
  line: string;
};

export function AttractScreen(props: AttractProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text role="caption">NoWayOS showcase - fictional brands, nothing kept</Text>
        <Spacer />
        <HeaderBar padding={TIGHT}>
          <Text id="title" role="title">
            {props.title}
          </Text>
        </HeaderBar>
        <Text role="heading">{props.line}</Text>
        <Spacer />
        <Notice tone="success" padding={TIGHT}>
          <Text weight="bold">Press any key or touch the screen to start</Text>
        </Notice>
      </Column>
    </Screen>
  );
}
