// The Screen Compiler gallery, 3 of 11: a text's weight, the lines it may wrap onto and where its lines sit.
// Each example is captioned with the TSX that made it. Written for the showcase in TSX and
// converted at build time like every other screen here; the launcher's Screen Compiler
// tile opens the first, and Back and Next go round them all.
import { Button, Card, Column, HeaderBar, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;

type GalleryProps = {
  onHome: () => void;
  onBack: () => void;
  onNext: () => void;
};

export function GalleryTextScreen(props: GalleryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={props.onHome}>
            Home
          </Button>
          <Text id="title" role="heading">
            Setting text
          </Text>
          <Spacer />
          <Text>
            3 of 11
          </Text>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </HeaderBar>
        <Text weight="bold">Bold body text</Text>
        <Text role="caption">{'<Text weight="bold">, or weight="regular" (the default):'}</Text>
        <Text weight="regular">Regular body text</Text>
        <Text lines={3}>
          A longer text wraps onto as many lines as it is given, breaking after a space at the width layout gives
          it, so this sentence takes a second line.
        </Text>
        <Text role="caption">{'<Text lines={3}>: at most three lines'}</Text>
        <Card padding={TIGHT}>
          <Text align="start">Start</Text>
          <Text align="center">Centre</Text>
          <Text align="end">End</Text>
        </Card>
        <Text role="caption">{'<Text align="start">, align="center" and align="end"'}</Text>
      </Column>
    </Screen>
  );
}
