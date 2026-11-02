// The Screen Compiler gallery, 2 of 11: every role of the theme's type scale. Display draws digits,
// capitals and a few signs, for a number read across a room.
// Each example is captioned with the TSX that made it. Written for the showcase in TSX and
// converted at build time like every other screen here; the launcher's Screen Compiler
// tile opens the first, and Back and Next go round them all.
import { Button, Column, HeaderBar, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;

type GalleryProps = {
  onHome: () => void;
  onBack: () => void;
  onNext: () => void;
};

export function GalleryTypeScreen(props: GalleryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={props.onHome}>
            Home
          </Button>
          <Text id="title" role="heading">
            Type scale
          </Text>
          <Spacer />
          <Text>
            2 of 11
          </Text>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </HeaderBar>
        <Row gap={EDGE}>
          <Column gap={4}>
            <Text role="display">A047</Text>
            <Text role="caption">{'<Text role="display">'}</Text>
          </Column>
          <Column gap={4}>
            <Text role="title">A screen's title</Text>
            <Text role="caption">{'<Text role="title">'}</Text>
            <Text role="heading">A heading</Text>
            <Text role="caption">{'<Text role="heading">'}</Text>
            <Text role="body">Body text, the default</Text>
            <Text role="caption">{'<Text role="body">'}</Text>
            <Text role="label">A field's label</Text>
            <Text role="caption">{'<Text role="label">'}</Text>
            <Text role="caption">Small print</Text>
            <Text role="caption">{'<Text role="caption">'}</Text>
          </Column>
        </Row>
      </Column>
    </Screen>
  );
}
