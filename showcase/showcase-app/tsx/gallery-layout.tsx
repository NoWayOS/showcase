// The Screen Compiler gallery, 1 of 11: columns and rows, their gap and padding - a number, a design token
// or one of the theme's spacing tokens - and spacers.
// Each example is captioned with the TSX that made it. Written for the showcase in TSX and
// converted at build time like every other screen here; the launcher's Screen Compiler
// tile opens the first, and Back and Next go round them all.
import { Button, Card, Column, HeaderBar, Row, Screen, Spacer, Text, space } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;

type GalleryProps = {
  onHome: () => void;
  onBack: () => void;
  onNext: () => void;
};

export function GalleryLayoutScreen(props: GalleryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={props.onHome}>
            Home
          </Button>
          <Text id="title" role="heading">
            Layout
          </Text>
          <Spacer />
          <Text>
            1 of 11
          </Text>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </HeaderBar>
        <Text role="caption">{'<Row gap={space.md} padding={4}>: children side by side'}</Text>
        <Row gap={space.md} padding={4}>
          <Card padding={TIGHT}>
            <Text>One</Text>
          </Card>
          <Card padding={TIGHT}>
            <Text>Two</Text>
          </Card>
          <Card padding={TIGHT}>
            <Text>Three</Text>
          </Card>
        </Row>
        <Text role="caption">{'<Column gap={4} padding={space.sm}>: children stacked'}</Text>
        <Card>
          <Column gap={4} padding={space.sm}>
            <Text>First</Text>
            <Text>Second</Text>
          </Column>
        </Card>
        <Text role="caption">{'<Spacer />: the room left over; <Spacer size={48} />: a fixed 48'}</Text>
        <Row gap={GAP}>
          <Text>Start</Text>
          <Spacer size={48} />
          <Text>48 on</Text>
          <Spacer />
          <Text>End</Text>
        </Row>
      </Column>
    </Screen>
  );
}
